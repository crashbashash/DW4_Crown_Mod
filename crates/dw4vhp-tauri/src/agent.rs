//! Opt-in local automation for the actual webview and native backend.
//! Compile with `agent-control`; set DW4_AGENT_TOKEN to start the server.
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tauri::{Manager, State};

type PendingReply = Option<(u64, mpsc::Sender<Value>)>;

#[derive(Default)]
pub struct AgentState(pub Arc<Mutex<PendingReply>>);

#[tauri::command]
pub fn agent_reply(id: u64, result: Value, state: State<'_, AgentState>) {
    let mut pending = state.0.lock().unwrap();
    if pending
        .as_ref()
        .is_some_and(|(expected, _)| *expected == id)
    {
        if let Some((_, sender)) = pending.take() {
            let _ = sender.send(result);
        }
    }
}

fn request(stream: &mut TcpStream, token: &str) -> Result<String, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    let end = loop {
        let mut chunk = [0; 4096];
        let n = stream.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("incomplete request".into());
        }
        bytes.extend_from_slice(&chunk[..n]);
        if bytes.len() > 1_048_576 {
            return Err("request too large".into());
        }
        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let headers = std::str::from_utf8(&bytes[..end]).map_err(|e| e.to_string())?;
    if !headers.starts_with("POST /eval HTTP/1.1\r\n") {
        return Err("use POST /eval".into());
    }
    let mut length = None;
    let mut authorized = false;
    for line in headers.lines().skip(1) {
        if let Some((key, value)) = line.split_once(':') {
            if key.eq_ignore_ascii_case("content-length") {
                length = value.trim().parse::<usize>().ok();
            }
            if key.eq_ignore_ascii_case("authorization") {
                authorized = value.trim() == format!("Bearer {token}");
            }
        }
    }
    if !authorized {
        return Err("unauthorized".into());
    }
    let length = length.ok_or("content-length required")?;
    if length > 1_000_000 {
        return Err("request too large".into());
    }
    while bytes.len() < end + length {
        let mut chunk = [0; 4096];
        let n = stream.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("incomplete body".into());
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
    let body: Value =
        serde_json::from_slice(&bytes[end..end + length]).map_err(|e| e.to_string())?;
    body["script"]
        .as_str()
        .map(str::to_owned)
        .ok_or("script required".into())
}

pub fn start(app: tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let Ok(token) = std::env::var("DW4_AGENT_TOKEN") else {
        return Ok(());
    };
    if token.len() < 24 {
        return Err("DW4_AGENT_TOKEN must have at least 24 characters".into());
    }
    let port = std::env::var("DW4_AGENT_PORT")
        .unwrap_or_else(|_| "17364".into())
        .parse::<u16>()?;
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let pending = app.state::<AgentState>().0.clone();
    std::thread::spawn(move || {
        for (index, mut stream) in listener.incoming().flatten().enumerate() {
            let id = index as u64 + 1;
            let result = request(&mut stream, &token).and_then(|script| {
                let (sender, receiver) = mpsc::channel();
                *pending.lock().unwrap() = Some((id, sender));
                let script = serde_json::to_string(&script).unwrap();
                let js = format!(
                    r#"(async () => {{
                    let result;
                    try {{ result = {{ok: true, value: (await (0,eval)({script})) ?? null}}; }}
                    catch (error) {{ result = {{ok: false, error: String(error)}}; }}
                    await window.__TAURI_INTERNALS__.invoke('agent_reply', {{id: {id}, result}});
                }})()"#
                );
                app.get_webview_window("main")
                    .ok_or("main window missing")?
                    .eval(&js)
                    .map_err(|e| e.to_string())?;
                receiver
                    .recv_timeout(Duration::from_secs(10))
                    .map_err(|e| e.to_string())
            });
            *pending.lock().unwrap() = None;
            let body = match result {
                Ok(value) => value,
                Err(error) => json!({"ok": false, "error": error}),
            }
            .to_string();
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
            let _ = stream.write_all(response.as_bytes());
        }
    });
    Ok(())
}
