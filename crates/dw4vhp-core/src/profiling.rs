//! Optional per-thread timings for the synchronous patch pipeline. Enable with
//! DW4_PROFILE=1. Timings never enter the generated image or change RNG state.
use std::{
    cell::RefCell,
    collections::BTreeMap,
    marker::PhantomData,
    path::Path,
    rc::Rc,
    sync::OnceLock,
    time::{Duration, Instant},
};

#[derive(Default)]
struct Total {
    calls: u64,
    inclusive: Duration,
    exclusive: Duration,
    maximum: Duration,
}
#[derive(Default)]
struct State {
    children: Vec<Duration>,
    totals: BTreeMap<String, Total>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("DW4_PROFILE").is_some())
}

#[must_use]
pub struct Span {
    started: Option<Instant>,
    name: std::borrow::Cow<'static, str>,
    _thread: PhantomData<Rc<()>>,
}

pub fn measure(name: &'static str) -> Span {
    start(name.into())
}
pub fn measure_owned(name: String) -> Span {
    start(name.into())
}
fn start(name: std::borrow::Cow<'static, str>) -> Span {
    let started = enabled().then(|| {
        STATE.with(|state| state.borrow_mut().children.push(Duration::ZERO));
        Instant::now()
    });
    Span {
        started,
        name,
        _thread: PhantomData,
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        let Some(started) = self.started else { return };
        let elapsed = started.elapsed();
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            let children = state.children.pop().unwrap_or_default();
            if let Some(parent) = state.children.last_mut() {
                *parent += elapsed;
            }
            let total = state.totals.entry(self.name.to_string()).or_default();
            total.calls += 1;
            total.inclusive += elapsed;
            total.exclusive += elapsed.saturating_sub(children);
            total.maximum = total.maximum.max(elapsed);
        });
    }
}

/// Write the current patch thread's totals after all measured work returns.
/// Inclusive times overlap; exclusive times exclude measured child calls.
pub fn write(path: &Path) -> std::io::Result<()> {
    let report = STATE.with(|state| {
        let state = state.borrow();
        let mut totals = state.totals.iter().collect::<Vec<_>>();
        totals.sort_by_key(|(_, t)| std::cmp::Reverse(t.exclusive));
        serde_json::json!({"enabled": enabled(), "spans": totals.into_iter().map(|(name, t)| {
            serde_json::json!({"name": name, "calls": t.calls,
                "inclusiveSeconds": t.inclusive.as_secs_f64(),
                "exclusiveSeconds": t.exclusive.as_secs_f64(),
                "maxSeconds": t.maximum.as_secs_f64()})
        }).collect::<Vec<_>>()})
    });
    let file = std::fs::File::create(path)?;
    serde_json::to_writer_pretty(file, &report).map_err(std::io::Error::other)
}
