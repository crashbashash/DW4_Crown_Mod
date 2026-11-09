//! Time the same vanilla-stat, seeded-randomizer pipeline used by the app.
//! Compile first, then time this executable; DW4_PROFILE=1 enables nested spans.
//! Usage: profile_iso <clean.iso> <out.iso> <report.json> [seed]
use dw4vhp_core::{
    patch::{patch_file, PatchOptions},
    plan::PatchPlan,
    profiling,
};
use std::{
    collections::BTreeMap,
    env,
    path::Path,
    time::{Duration, Instant},
};
fn main() {
    let args = env::args().collect::<Vec<_>>();
    assert!(
        args.len() >= 4,
        "profile_iso <clean.iso> <out.iso> <report.json> [seed]"
    );
    let seed = args.get(4).map(|s| s.parse().unwrap()).unwrap_or(1520);
    let plan = PatchPlan {
        crown_mod: false,
        random_enemies: true,
        enemy_seed: Some(seed),
        ..PatchPlan::default()
    };
    let started = Instant::now();
    let mut phase = None;
    let mut phase_start = started;
    let mut phases: BTreeMap<String, Duration> = BTreeMap::new();
    let outcome = patch_file(
        Path::new(&args[1]),
        Path::new(&args[2]),
        &plan,
        &PatchOptions { overwrite: false },
        &mut |p| {
            if phase != Some(p.phase) {
                let now = Instant::now();
                if let Some(previous) = phase {
                    *phases.entry(format!("{previous:?}")).or_default() += now - phase_start;
                }
                eprintln!("{:.3}s {:?}", started.elapsed().as_secs_f64(), p.phase);
                phase = Some(p.phase);
                phase_start = now;
            }
        },
    );
    if let Some(previous) = phase {
        *phases.entry(format!("{previous:?}")).or_default() += phase_start.elapsed();
    }
    profiling::write(Path::new(&args[3])).unwrap();
    let outcome = outcome.expect("build and verification");
    let mut report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&args[3]).unwrap()).unwrap();
    report["wallSeconds"] = started.elapsed().as_secs_f64().into();
    report["seed"] = seed.into();
    report["bytes"] = outcome.bytes.into();
    report["md5"] = outcome.md5.into();
    report["phases"] = serde_json::to_value(
        phases
            .into_iter()
            .map(|(k, v)| (k, v.as_secs_f64()))
            .collect::<BTreeMap<_, _>>(),
    )
    .unwrap();
    std::fs::write(&args[3], serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    println!(
        "verified {} bytes in {:.3}s; MD5 {}",
        report["bytes"],
        report["wallSeconds"].as_f64().unwrap(),
        report["md5"]
    );
}
