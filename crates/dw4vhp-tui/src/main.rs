//! Terminal front end for consoles without the desktop app's WebView, such as
//! Windows 7. Pass a disc image path (or drop one onto the executable) to start
//! with it checked.
mod app;
mod ui;

use app::{Action, App, Msg};
use dw4vhp_core::disc::inspect_retail_for_plan;
use dw4vhp_core::options::PatchSelection;
use dw4vhp_core::patch::{patch_file, PatchOptions};
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

fn main() -> std::io::Result<()> {
    let initial = std::env::args_os().nth(1).map(PathBuf::from);
    // crossterm writes ANSI escapes whenever TERM is set, even when the console
    // refused virtual-terminal mode. The Windows 7 console always refuses it and
    // would print the escapes as text, so let only the console decide.
    #[cfg(windows)]
    std::env::remove_var("TERM");
    let mut terminal = ratatui::try_init()?;
    let result = run(&mut terminal, initial);
    ratatui::restore();
    result
}

fn run(terminal: &mut DefaultTerminal, initial: Option<PathBuf>) -> std::io::Result<()> {
    let (tx, rx): (Sender<Msg>, Receiver<Msg>) = channel();
    let mut app = App::new();
    if let Some(path) = initial {
        let action = app.open(&path);
        perform(action, &tx);
    }
    loop {
        terminal.draw(|frame| ui::draw(frame, &app))?;
        // Redraw at most ten times a second: the Windows 7 console API is slow.
        if event::poll(Duration::from_millis(100))? {
            // Windows reports key releases too; act on presses only.
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Release {
                    match app.key(key) {
                        Action::Quit => return Ok(()),
                        action => perform(action, &tx),
                    }
                }
            }
        }
        while let Ok(msg) = rx.try_recv() {
            app.receive(msg);
        }
    }
}

fn perform(action: Action, tx: &Sender<Msg>) {
    let tx = tx.clone();
    match action {
        Action::None | Action::Quit => {}
        Action::Check(path) => {
            std::thread::spawn(move || {
                let plan = PatchSelection::default().analysis_plan();
                let result = inspect_retail_for_plan(&path, &plan)
                    .map_err(|error| error.disc_notice())
                    .and_then(|report| {
                        let release = report
                            .release
                            .ok_or_else(|| dw4vhp_core::error::Error::NotThisDisc.disc_notice())?;
                        Ok(format!("{} - {}", release.label(), release.serial()))
                    });
                let _ = tx.send(Msg::Checked(result));
            });
        }
        Action::Patch {
            input,
            output,
            selection,
            overwrite,
        } => {
            std::thread::spawn(move || {
                let result = selection
                    .into_plan()
                    .map_err(str::to_owned)
                    .and_then(|plan| {
                        let progress_tx = tx.clone();
                        patch_file(
                            &input,
                            &output,
                            &plan,
                            &PatchOptions { overwrite },
                            &mut |p| {
                                let _ = progress_tx.send(Msg::Progress(p));
                            },
                        )
                        .map_err(|error| error.to_string())
                    });
                let _ = tx.send(Msg::Finished(result));
            });
        }
    }
}
