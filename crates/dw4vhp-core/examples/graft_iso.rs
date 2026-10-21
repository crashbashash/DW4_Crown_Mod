//! Experimental: build a PS2 USA image whose Abyss of Grief spawns Mummymon.
//! Usage: graft_iso <clean.iso> <output.iso> [control|ogre|plain|stressN]
//! `stress<N>` grafts the N largest regular enemies (memory stress test).
//! `list:entry:model,...` grafts exactly those models.
//! `control` builds the same relocated archive with no graft, for A/B tests.
use dw4vhp_core::crown::{
    graft::{Spec, INTRO_STAGES},
    CrownPlan,
};
use std::{env, fs, path::Path, process::exit};

/// Regular enemies by descending size: (source entry, model).
const STRESS: &[(usize, &str)] = &[
    (723, "e_infermo"),
    (187, "e_nume"),
    (296, "e_tonogeko"),
    (187, "e_scum"),
    (425, "e_ldknight"),
    (441, "e_mammos"),
    (635, "e_cockatri"),
    (335, "e_maridevi"),
    (723, "e_impdra"),
    (349, "e_diabolo"),
    (394, "e_rare"),
    (349, "e_geko"),
    (257, "e_mummy"),
    (265, "e_anomalo"),
    (319, "e_mecha4"),
    (642, "e_tyrano"),
    (257, "e_minotaru"),
    (257, "e_pharaoh"),
    (394, "e_mecha5"),
    (679, "e_torta"),
    (387, "e_megadra"),
    (312, "e_mecha3"),
];

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 || args.len() > 4 {
        eprintln!("usage: graft_iso <clean.iso> <output.iso> [control|ogre|plain|stressN]");
        exit(2);
    }
    let (input, output) = (Path::new(&args[1]), Path::new(&args[2]));
    if output.exists() {
        eprintln!("refusing to overwrite {}", output.display());
        exit(2);
    }
    let iso = fs::read(input).expect("read input");
    let plan = if args.get(3).is_some_and(|a| a == "control") {
        CrownPlan::build(&iso).expect("build control plan")
    } else if let Some(list) = args.get(3).and_then(|a| a.strip_prefix("list:")) {
        let models: Vec<(usize, &str)> = list
            .split(',')
            .map(|x| {
                let (e, m) = x.split_once(':').expect("entry:model");
                (e.parse().expect("entry"), m)
            })
            .collect();
        let families = env::var("DW4_FAMILIES").unwrap_or_else(|_| "e_goburi,e_blosso,e_ogre".into());
        let families: Vec<&str> = families.split(',').collect();
        CrownPlan::build_graft(&iso, &Spec::abyss_stress_families(&models, &families)).expect("build list plan")
    } else if let Some(n) = args.get(3).and_then(|a| a.strip_prefix("stress")) {
        let n: usize = n.parse().expect("stress<N>");
        CrownPlan::build_graft(&iso, &Spec::abyss_stress_families(&STRESS[..n], &["e_goburi"])).expect("build stress plan")
    } else if let Some(m) = args.get(3).and_then(|a| a.strip_prefix("intro:")) {
        // intro:<source entry>:<model>[:lua]
        let mut it = m.split(':');
        let entry: usize = it.next().and_then(|e| e.parse().ok()).expect("source entry");
        let model = it.next().expect("model");
        let spec = Spec::abyss_intro(entry, model, it.next() == Some("lua"));
        CrownPlan::build_graft(&iso, &spec).expect("build intro plan")
    } else if let Some(m) = args.get(3).and_then(|a| a.strip_prefix("stage:")) {
        // stage:<model entry>:<warp target|->:<lua|nolua>:<orig>=<src>/<donor>[,<orig>=<src>/<donor>...]
        let f: Vec<&str> = m.splitn(4, ':').collect();
        let entry: usize = f[0].parse().expect("model entry");
        let stage = INTRO_STAGES.iter().find(|s| s.model_entry == entry).expect("unknown stage");
        let warp = (f[1] != "-").then_some(f[1]);
        let picks: Vec<(&str, usize, &str)> = f[3]
            .split(',')
            .map(|p| {
                let (orig, rest) = p.split_once('=').expect("orig=src/donor");
                let (src, donor) = rest.split_once('/').expect("src/donor");
                (orig, src.parse().expect("src entry"), donor)
            })
            .collect();
        let spec = Spec::intro_stage(stage, &picks, f[2] == "lua", warp).expect("stage spec");
        CrownPlan::build_graft(&iso, &spec).expect("build stage plan")
    } else if let Some(m) = args.get(3).and_then(|a| a.strip_prefix("families:")) {
        // families:<intro entry>:<intro model>:<goburi entry>:<goburi model>[:lua]
        let f: Vec<&str> = m.split(':').collect();
        let e = |i: usize| f[i].parse::<usize>().expect("entry");
        let spec = Spec::abyss_intro_families((e(0), f[1]), (e(2), f[3]), f.get(4) == Some(&"lua"));
        CrownPlan::build_graft(&iso, &spec).expect("build families plan")
    } else if args.get(3).is_some_and(|a| a == "plain") {
        CrownPlan::build_graft(&iso, &Spec::abyss_mummy_plain()).expect("build plain graft plan")
    } else if args.get(3).is_some_and(|a| a == "ogre") {
        CrownPlan::build_graft(&iso, &Spec::abyss_ogre()).expect("build ogre control plan")
    } else {
        CrownPlan::build_graft(&iso, &Spec::abyss_mummy()).expect("build graft plan")
    };
    let mut file = fs::File::create_new(output).expect("create output");
    plan.write_image(&iso, &mut file).expect("write image");
    drop(file);
    let out = fs::read(output).expect("re-read output");
    plan.verify(&iso, &out).expect("verify output");
    println!("wrote {} ({} bytes)", output.display(), out.len());
}
