//! Optional full-disc check of the experimental enemy randomiser. No game data is included.
use dw4vhp_core::crown::{CrownPlan, RandomOptions};
use std::path::PathBuf;

fn options(seed: u64) -> RandomOptions {
    RandomOptions {
        seed,
        donors: 3,
        warp: None,
        only: None,
    }
}

#[test]
fn randomised_image_is_deterministic_and_verifies() {
    let Some(path) = std::env::var_os("DW4_RETAIL_USA_ISO").map(PathBuf::from) else {
        eprintln!("SKIP: DW4_RETAIL_USA_ISO not configured");
        return;
    };
    let iso = std::fs::read(path).unwrap();
    let a = CrownPlan::build_random(&iso, options(7)).unwrap();
    let b = CrownPlan::build_random(&iso, options(7)).unwrap();
    let c = CrownPlan::build_random(&iso, options(8)).unwrap();
    assert_eq!(a.output_size, b.output_size, "same seed, same image");
    assert_ne!(a.output_size, c.output_size, "another seed, another image");
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("random.iso");
    let mut file = std::fs::File::create_new(&out).unwrap();
    a.write_image(&iso, &mut file).unwrap();
    drop(file);
    let written = std::fs::read(&out).unwrap();
    assert_eq!(written.len() as u64, a.output_size);
    a.verify(&iso, &written).unwrap();
}
