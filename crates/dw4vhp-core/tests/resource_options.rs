//! Opt-in full-disc coverage for the current balance and optional settings.
//! Supply local retail paths through the environment; no game data is bundled.
use dw4vhp_core::{
    container::{self, Format},
    disc::inspect_retail_with_options,
    patch::{patch_file, PatchOptions},
    plan::{transform, PatchPlan},
    ratio::RatioVector,
    release::{self, Release},
};
use md5::{Digest, Md5};
use memmap2::Mmap;
use std::{fs::File, io::Read, path::Path};

fn digest(path: &Path) -> String {
    let mut file = File::open(path).unwrap();
    let mut hash = Md5::new();
    let mut buf = vec![0; 1024 * 1024];
    loop {
        let len = file.read(&mut buf).unwrap();
        if len == 0 {
            break;
        }
        hash.update(&buf[..len]);
    }
    format!("{:x}", hash.finalize())
}

fn check(key: &str, expected_release: Release, expected_format: Format) -> bool {
    let Some(path) = std::env::var_os(key) else {
        return false;
    };
    let input = Path::new(&path);
    assert_eq!(container::detect(input).unwrap(), expected_format);
    let input_hash = digest(input);
    let combinations: &[(bool, bool, bool)] = if expected_format == Format::Raw {
        &[
            (false, false, false),
            (true, false, false),
            (false, true, false),
            (true, true, false),
            (true, true, true),
        ]
    } else {
        // The four independent switch combinations are covered on every raw
        // release. Compressed cases exercise both switches through unpack/pack.
        &[(true, true, false), (true, true, true)]
    };
    for &(four_player_spawns, crown_drops, random_crown_colours) in combinations {
        let report = inspect_retail_with_options(input, four_player_spawns, crown_drops).unwrap();
        assert_eq!(report.release, Some(expected_release));
        let plan = PatchPlan {
            four_player_spawns,
            crown_drops,
            random_crown_colours,
            crown_colour_seed: Some(0x72abcdef),
            ..PatchPlan::default()
        };
        let (expected_table, _) = transform(
            &report.authored,
            &plan,
            &RatioVector::derive(&report.authored),
        );
        let blocks = expected_table.blocks();
        assert_eq!(
            format!("{:x}", Md5::digest(&blocks[1])),
            "c2cdbb0ca237b6619d55374bd4ed86e5",
            "all supported releases use the same half-Speed practice balance"
        );
        let tmp = tempfile::tempdir().unwrap();
        let output = tmp.path().join(match expected_format {
            Format::Raw => "patched.iso",
            Format::Ciso => "patched.ciso",
            Format::Rvz => "patched.rvz",
        });
        let mut progress_events = Vec::new();
        let outcome = patch_file(
            input,
            &output,
            &plan,
            &PatchOptions { overwrite: false },
            &mut |p| progress_events.push(p),
        )
        .unwrap();
        assert_eq!(progress_events.first().unwrap().done, 0);
        assert_eq!(progress_events.last().unwrap().done, 10000);
        assert!(progress_events
            .iter()
            .all(|p| p.total == 10000 && p.done <= p.total));
        assert!(
            progress_events
                .windows(2)
                .all(|pair| pair[0].done <= pair[1].done),
            "overall progress must not restart between stages"
        );
        assert_eq!(outcome.summary.rows_changed, 640);
        assert_eq!(outcome.bytes, output.metadata().unwrap().len());
        assert_eq!(container::detect(&output).unwrap(), expected_format);
        let plain = if expected_format == Format::Raw {
            assert_eq!(outcome.bytes, report.output_size);
            output.clone()
        } else {
            let plain = tmp.path().join("verify.iso");
            container::unpack(expected_format, &output, &plain, &mut |_, _| {}).unwrap();
            assert!(plain.metadata().unwrap().len() >= report.output_size);
            plain
        };
        let file = File::open(&plain).unwrap();
        // Safety: the completed output is read-only throughout verification.
        let image = unsafe { Mmap::map(&file).unwrap() };
        assert_eq!(release::detect(&image).unwrap(), expected_release);
        let layout = expected_release.layout_in(&image).unwrap();
        for (block, offset) in blocks
            .iter()
            .zip([layout.hp_off, layout.stat_off, layout.byte_off])
        {
            assert_eq!(
                &image[offset as usize..offset as usize + block.len()],
                block
            );
            assert_eq!(memchr::memmem::find_iter(&image, block).count(), 665);
        }
        assert!(image[report.output_size as usize..].iter().all(|b| *b == 0));
        eprintln!(
            "PASS: {key} spawns={four_player_spawns} drops={crown_drops} colours={random_crown_colours} bytes={} md5={}; all 1995 current-balance blocks verified",
            outcome.bytes, outcome.md5
        );
    }
    assert_eq!(digest(input), input_hash, "retail input must be unchanged");
    true
}

#[test]
fn supported_releases_and_containers_keep_all_resource_options() {
    let mut configured = 0;
    for (key, release, format) in [
        ("DW4_RETAIL_USA_ISO", Release::Usa, Format::Raw),
        ("DW4_RETAIL_EUROPE_ISO", Release::Europe, Format::Raw),
        ("DW4_RETAIL_JAPAN_ISO", Release::Japan, Format::Raw),
        ("DW4_RETAIL_GC_USA_ISO", Release::GcUsa, Format::Raw),
        ("DW4_RETAIL_GC_JAPAN_ISO", Release::GcJapan, Format::Raw),
        ("DW4_RETAIL_GC_USA_CISO", Release::GcUsa, Format::Ciso),
        ("DW4_RETAIL_GC_JAPAN_CISO", Release::GcJapan, Format::Ciso),
        ("DW4_RETAIL_GC_USA_RVZ", Release::GcUsa, Format::Rvz),
        ("DW4_RETAIL_GC_JAPAN_RVZ", Release::GcJapan, Format::Rvz),
    ] {
        configured += usize::from(check(key, release, format));
    }
    eprintln!("Resource-option matrix: {configured}/9 retail inputs configured");
}
