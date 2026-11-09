//! Analysis: the enemy randomiser's choices for many seeds, without building images.
//! Usage: random_picks <clean image> <output dir> <threads> <bosses 0|1> <seed>...
//! Writes `<dir>/<seed>.json`. Picks match an image built with the same settings:
//! Crown Mod and the other options off, unless `DW4_PICKS_CROWN=1`.
//! With `DW4_PROFILE=1 PROF_OUT=<file>`, each worker writes its timings there on exit
//! (the last worker to finish wins); use one thread for a whole-run profile.
use dw4vhp_core::crown::{CrownPlan, RandomOptions, RANDOM_DONORS};
use std::{
    env, fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

fn main() {
    let args: Vec<String> = env::args().collect();
    assert!(
        args.len() >= 6,
        "random_picks <image> <dir> <threads> <bosses 0|1> <seed>..."
    );
    let iso = Arc::new(fs::read(&args[1]).expect("read image"));
    let dir = PathBuf::from(&args[2]);
    let threads: usize = args[3].parse().expect("threads");
    let bosses = args[4] == "1";
    let crown = env::var("DW4_PICKS_CROWN").as_deref() == Ok("1");
    let seeds: Vec<u64> = args[5..].iter().map(|s| s.parse().expect("seed")).collect();
    let queue = Arc::new(Mutex::new(seeds));
    let workers: Vec<_> = (0..threads)
        .map(|_| {
            let (iso, queue, dir) = (iso.clone(), queue.clone(), dir.clone());
            std::thread::spawn(move || loop {
                let Some(seed) = queue.lock().unwrap().pop() else {
                    if let Ok(out) = env::var("PROF_OUT") {
                        dw4vhp_core::profiling::write(std::path::Path::new(&out)).unwrap();
                    }
                    break;
                };
                let started = std::time::Instant::now();
                let options = RandomOptions {
                    seed,
                    random_bosses: bosses,
                    donors: RANDOM_DONORS,
                    warp: None,
                    only: None,
                    picks_only: true,
                };
                match CrownPlan::random_picks(&iso, false, false, false, 0, options, crown) {
                    Ok(picks) => {
                        fs::write(
                            dir.join(format!("{seed}.json")),
                            serde_json::to_vec(&picks).unwrap(),
                        )
                        .unwrap();
                        println!("ok {seed} {:.1}", started.elapsed().as_secs_f64());
                    }
                    Err(e) => println!("FAIL {seed} {e}"),
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
}
