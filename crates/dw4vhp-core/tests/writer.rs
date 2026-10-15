use dw4vhp_core::error::Error;
use dw4vhp_core::region::WriteRegion;
use dw4vhp_core::writer::{write_output, Phase, WriteOptions};
use std::io::Write;

fn fixture(dir: &tempfile::TempDir, bytes: &[u8]) -> std::path::PathBuf {
    let p = dir.path().join("in.iso");
    let mut f = std::fs::File::create(&p).unwrap();
    f.write_all(bytes).unwrap();
    p
}
fn noop(_: dw4vhp_core::writer::Progress) {}
fn ok(_: &std::path::Path) -> dw4vhp_core::error::Result<()> {
    Ok(())
}

#[test]
fn refuses_when_the_destination_has_less_space_than_the_copy_needs() {
    assert!(dw4vhp_core::writer::ensure_space(1_448_902_656, 1_000).is_err());
    assert!(dw4vhp_core::writer::ensure_space(1_448_902_656, 2_000_000_000).is_ok());
}

#[test]
fn copies_the_input_and_applies_regions_without_touching_the_input() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(&dir, b"0123456789");
    let out = dir.path().join("out.iso");
    let r = write_output(
        &input,
        &out,
        &[WriteRegion {
            offset: 2,
            bytes: b"XY".to_vec(),
        }],
        &WriteOptions { overwrite: false },
        &mut noop,
        &ok,
    )
    .unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), b"01XY456789");
    assert_eq!(std::fs::read(&input).unwrap(), b"0123456789"); // input untouched
    assert_eq!(r.bytes, 10);
    assert_eq!(r.md5.len(), 32);
    assert!(!dir.path().join("out.iso.part").exists());
}

#[test]
fn refuses_when_the_output_is_the_input() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(&dir, b"0123456789");
    let err = write_output(
        &input,
        &input.clone(),
        &[],
        &WriteOptions { overwrite: true },
        &mut noop,
        &ok,
    )
    .unwrap_err();
    assert!(matches!(err, Error::OutputIsInput), "{err:?}");
    assert_eq!(std::fs::read(&input).unwrap(), b"0123456789"); // still intact
}

#[test]
fn refuses_an_existing_output_unless_asked_to_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(&dir, b"0123456789");
    let out = dir.path().join("out.iso");
    std::fs::write(&out, b"existing").unwrap();
    assert!(matches!(
        write_output(
            &input,
            &out,
            &[],
            &WriteOptions { overwrite: false },
            &mut noop,
            &ok
        )
        .unwrap_err(),
        Error::OutputExists
    ));
    write_output(
        &input,
        &out,
        &[],
        &WriteOptions { overwrite: true },
        &mut noop,
        &ok,
    )
    .unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), b"0123456789");
}

#[test]
fn works_when_the_input_is_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(&dir, b"0123456789");
    let mut perms = std::fs::metadata(&input).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&input, perms).unwrap();
    let out = dir.path().join("out.iso");
    write_output(
        &input,
        &out,
        &[],
        &WriteOptions { overwrite: false },
        &mut noop,
        &ok,
    )
    .unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), b"0123456789");
}

#[test]
fn a_stale_part_file_is_replaced_not_trusted() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(&dir, b"0123456789");
    let out = dir.path().join("out.iso");
    std::fs::write(
        dir.path().join("out.iso.part"),
        b"garbage-from-an-interrupted-run",
    )
    .unwrap();
    write_output(
        &input,
        &out,
        &[],
        &WriteOptions { overwrite: false },
        &mut noop,
        &ok,
    )
    .unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), b"0123456789");
    assert!(!dir.path().join("out.iso.part").exists());
}

#[test]
fn a_failed_verification_leaves_nothing_at_the_output_path() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(&dir, b"0123456789");
    let out = dir.path().join("out.iso");
    let bad = |_: &std::path::Path| Err(Error::VerificationFailed("nope".into()));
    let err = write_output(
        &input,
        &out,
        &[],
        &WriteOptions { overwrite: false },
        &mut noop,
        &bad,
    )
    .unwrap_err();
    assert!(matches!(err, Error::VerificationFailed(_)));
    assert!(!out.exists(), "no half-written output may remain");
    assert!(
        !dir.path().join("out.iso.part").exists(),
        "the part file must be cleaned up"
    );
}

#[test]
fn reports_the_copy_and_write_phases() {
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(&dir, b"0123456789");
    let out = dir.path().join("out.iso");
    let mut phases = Vec::new();
    let mut rec = |p: dw4vhp_core::writer::Progress| phases.push(p.phase);
    write_output(
        &input,
        &out,
        &[WriteRegion {
            offset: 0,
            bytes: b"Z".to_vec(),
        }],
        &WriteOptions { overwrite: false },
        &mut rec,
        &ok,
    )
    .unwrap();
    assert!(phases.contains(&Phase::Copying));
    assert!(phases.contains(&Phase::Writing));
    assert!(phases.contains(&Phase::Verifying));
}

#[test]
fn streamed_extension_is_verified_before_replacing_existing_output() {
    use dw4vhp_core::writer::{write_output_extended, Extension};
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let input = fixture(&dir, b"original");
    let out = dir.path().join("out.iso");
    std::fs::write(&out, b"previous output").unwrap();
    let append = |file: &mut std::fs::File, _: &mut dyn FnMut(dw4vhp_core::writer::Progress)| {
        file.write_all(b"archive")?;
        Ok(())
    };
    let bad = |path: &std::path::Path| {
        assert_eq!(std::fs::read(path).unwrap(), b"originalarchive");
        Err(Error::VerificationFailed(
            "injected verification failure".into(),
        ))
    };
    assert!(write_output_extended(
        &input,
        &out,
        &[],
        &WriteOptions { overwrite: true },
        &mut noop,
        &bad,
        Some(Extension {
            output_size: 15,
            append: &append
        })
    )
    .is_err());
    assert_eq!(std::fs::read(&out).unwrap(), b"previous output");
    assert_eq!(std::fs::read(&input).unwrap(), b"original");
    assert!(!dir.path().join("out.iso.part").exists());
    let good = |path: &std::path::Path| {
        assert_eq!(std::fs::read(path).unwrap(), b"originalarchive");
        Ok(())
    };
    let result = write_output_extended(
        &input,
        &out,
        &[],
        &WriteOptions { overwrite: true },
        &mut noop,
        &good,
        Some(Extension {
            output_size: 15,
            append: &append,
        }),
    )
    .unwrap();
    assert_eq!(result.bytes, 15);
    assert_eq!(std::fs::read(&out).unwrap(), b"originalarchive");
    // An interrupted archive write must also preserve the previous output.
    let interrupted = |file: &mut std::fs::File,
                       _: &mut dyn FnMut(dw4vhp_core::writer::Progress)| {
        file.write_all(b"partial")?;
        Err(Error::VerificationFailed("injected append failure".into()))
    };
    assert!(write_output_extended(
        &input,
        &out,
        &[],
        &WriteOptions { overwrite: true },
        &mut noop,
        &ok,
        Some(Extension {
            output_size: 15,
            append: &interrupted
        })
    )
    .is_err());
    assert_eq!(std::fs::read(&out).unwrap(), b"originalarchive");
    assert!(!dir.path().join("out.iso.part").exists());
}
