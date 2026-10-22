//! Experimental: build a PS2 USA image with randomised enemies.
//! Usage: randomise_iso <clean.iso> <output.iso> <seed> [donors] [warp stage|-]
//! `DW4_RANDOM_ONLY=179,195` randomises just those model packs (empty: none).
use dw4vhp_core::crown::{CrownPlan, RandomOptions};
use std::{env, fs, path::Path, process::exit};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!(
            "usage: randomise_iso <clean.iso> <output.iso> <seed> [donors] [warp stage|-]
//! `DW4_RANDOM_ONLY=179,195` randomises just those model packs (empty: none)."
        );
        exit(2);
    }
    let (input, output) = (Path::new(&args[1]), Path::new(&args[2]));
    if output.exists() {
        eprintln!("refusing to overwrite {}", output.display());
        exit(2);
    }
    let options = RandomOptions {
        seed: args[3].parse().expect("seed"),
        donors: args.get(4).map_or(3, |d| d.parse().expect("donors")),
        warp: args.get(5).filter(|w| *w != "-").cloned(),
        only: env::var("DW4_RANDOM_ONLY")
            .ok()
            .map(|v| v.split(',').filter_map(|e| e.parse().ok()).collect()),
    };
    let iso = fs::read(input).expect("read input");
    let plan = CrownPlan::build_random(&iso, options).expect("build plan");
    let mut file = fs::File::create_new(output).expect("create output");
    plan.write_image(&iso, &mut file).expect("write image");
    drop(file);
    if env::var("DW4_CROWN_STATS").is_ok() {
        // The shipped Very Hard Plus enemy table on top of the randomised image.
        use std::io::{Seek, SeekFrom, Write};
        let report = dw4vhp_core::disc::inspect_retail_bytes(&iso).expect("inspect");
        let layout = dw4vhp_core::release::detect(&iso)
            .expect("release")
            .layout();
        let plan = dw4vhp_core::plan::PatchPlan::default();
        let (regions, _) =
            dw4vhp_core::patch::plan_regions(&iso, &report, &plan, &layout).expect("regions");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(output)
            .expect("open output");
        for r in &regions {
            file.seek(SeekFrom::Start(r.offset)).expect("seek");
            file.write_all(&r.bytes).expect("write");
        }
        println!("wrote {} stat regions", regions.len());
    }
    let out = fs::read(output).expect("re-read output");
    if env::var("DW4_CROWN_STATS").is_err() {
        plan.verify(&iso, &out).expect("verify output");
    }
    println!("wrote {} ({} bytes)", output.display(), out.len());
}
