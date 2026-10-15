use dw4vhp_core::{
    disc::inspect_retail_bytes,
    error::Error,
    iso9660,
    release::{detect, Release},
    testkit,
};

fn fixture(release: Release) -> Vec<u8> {
    let mut iso = testkit::iso_with_elf_and_cnf(release.serial());
    let elf = iso9660::find_file(&iso, "SLUS_208.36").unwrap();
    iso9660::rewrite_name(&mut iso, &elf, &format!("{};1", release.serial())).unwrap();
    iso
}

#[test]
fn detects_all_three_boot_identities() {
    for release in Release::ALL {
        assert_eq!(detect(&fixture(release)).unwrap(), release);
        let layout = release.layout();
        assert_eq!((layout.rows, layout.expected_copies), (649, 665));
        // A matching serial alone cannot make synthetic tables retail data.
        assert!(inspect_retail_bytes(&fixture(release)).is_err());
    }
}

#[test]
fn refuses_boot_config_and_directory_mismatches() {
    let iso = testkit::iso_with_elf_and_cnf("SLES_534.05");
    assert!(detect(&iso).is_err());
    assert!(detect(&testkit::iso_with_elf_and_cnf("SLUS_000.00")).is_err());
    assert!(detect(&testkit::iso_with_elf()).is_err());
}

#[test]
fn refuses_truncated_or_non_elf_boot_extents() {
    let mut iso = fixture(Release::Japan);
    let elf = iso9660::find_file(&iso, Release::Japan.serial()).unwrap();
    iso[elf.lba as usize * 2048] = 0;
    assert!(matches!(detect(&iso), Err(Error::NotThisDisc)));
    let mut iso = fixture(Release::Europe);
    let cnf = iso9660::find_file(&iso, "SYSTEM.CNF").unwrap();
    iso.truncate(cnf.lba as usize * 2048 + cnf.size as usize - 1);
    assert!(detect(&iso).is_err());
}

#[test]
fn refuses_duplicate_boot_settings() {
    let mut iso = fixture(Release::Usa);
    let cnf = iso9660::find_file(&iso, "SYSTEM.CNF").unwrap();
    let line = b"BOOT2=cdrom0:\\SLUS_208.36;1\n";
    let start = cnf.lba as usize * 2048;
    iso.resize(start + line.len() * 2, 0);
    iso[start..start + line.len()].copy_from_slice(line);
    iso[start + line.len()..].copy_from_slice(line);
    let size = (line.len() as u32 * 2).to_le_bytes();
    let at = cnf.record_offset as usize + 10;
    iso[at..at + 4].copy_from_slice(&size);
    assert!(matches!(detect(&iso), Err(Error::NotThisDisc)));
}

#[test]
fn non_usa_reports_never_apply_the_usa_difficulty_instruction() {
    use dw4vhp_core::{disc::inspect_bytes, patch::plan_regions, plan::PatchPlan};
    let (_dir, path, layout) = testkit::full_disc_tempfile();
    let iso = std::fs::read(path).unwrap();
    let mut report = inspect_bytes(&iso, &layout).unwrap();
    let plan = PatchPlan {
        force_very_hard: true,
        ..PatchPlan::default()
    };
    for release in [Release::Europe, Release::Japan] {
        report.release = Some(release);
        assert!(matches!(
            plan_regions(&iso, &report, &plan, &layout),
            Err(Error::UnsupportedDifficulty)
        ));
    }
}
