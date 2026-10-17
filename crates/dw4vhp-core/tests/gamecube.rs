use dw4vhp_core::container::{detect, pack, unpack, Format};
use dw4vhp_core::error::Error;
use dw4vhp_core::gcm;
use dw4vhp_core::release::{detect as detect_release, Release};

/// A small GameCube image: header, an FST with two files, and their bytes.
fn disc(id: &[u8; 6]) -> Vec<u8> {
    let mut disc = vec![0u8; 0x2000];
    disc[..6].copy_from_slice(id);
    disc[0x1C..0x20].copy_from_slice(&[0xC2, 0x33, 0x9F, 0x3D]);
    let fst = 0x1000usize;
    let names = b"area.afs\0ndmw.afs\0";
    let table_len = 3 * 12 + names.len();
    disc[0x424..0x428].copy_from_slice(&(fst as u32).to_be_bytes());
    disc[0x428..0x42C].copy_from_slice(&(table_len as u32).to_be_bytes());
    disc[fst + 8..fst + 12].copy_from_slice(&3u32.to_be_bytes());
    for (i, (name_at, offset, size)) in [(0u32, 0x1800u32, 0x100u32), (9, 0x1900, 0x200)]
        .into_iter()
        .enumerate()
    {
        let at = fst + 12 * (i + 1);
        disc[at + 1..at + 4].copy_from_slice(&name_at.to_be_bytes()[1..]);
        disc[at + 4..at + 8].copy_from_slice(&offset.to_be_bytes());
        disc[at + 8..at + 12].copy_from_slice(&size.to_be_bytes());
    }
    disc[fst + 36..fst + 36 + names.len()].copy_from_slice(names);
    disc
}

#[test]
fn the_fst_lookup_finds_files_ignoring_case() {
    let disc = disc(b"GDJEB2");
    let afs = gcm::find_file(&disc, "NDMW.AFS").unwrap();
    assert_eq!((afs.offset, afs.size), (0x1900, 0x200));
    assert_eq!(afs.offset_field, 0x1000 + 24 + 4);
    assert!(matches!(
        gcm::find_file(&disc, "missing.afs"),
        Err(Error::FileNotFound { .. })
    ));
}

#[test]
fn only_gamecube_images_have_a_game_id() {
    assert!(gcm::game_id(&vec![0u8; 0x2000]).is_err());
    assert_eq!(gcm::game_id(&disc(b"GDJJB2")).unwrap(), b"GDJJB2");
}

#[test]
fn releases_are_told_apart_by_game_id() {
    assert_eq!(detect_release(&disc(b"GDJEB2")).unwrap(), Release::GcUsa);
    assert_eq!(detect_release(&disc(b"GDJJB2")).unwrap(), Release::GcJapan);
    assert!(detect_release(&disc(b"GDJPB2")).is_err());
    assert!(detect_release(&disc(b"GALE01")).is_err());
}

#[test]
fn layout_follows_the_area_archive() {
    let mut disc = disc(b"GDJEB2");
    let base = Release::GcUsa.layout_in(&disc).unwrap().hp_off;
    // Move area.afs 0x100 bytes later: the tables move with it.
    let fst = 0x1000 + 12 + 4;
    disc[fst..fst + 4].copy_from_slice(&(0x1800u32 + 0x100).to_be_bytes());
    let moved = Release::GcUsa.layout_in(&disc).unwrap();
    assert_eq!(moved.hp_off, base + 0x100);
    assert_eq!(moved.stat_off - moved.hp_off, 0x47F4);
}

fn roundtrip(format: Format, name: &str) {
    let dir = tempfile::tempdir().unwrap();
    let raw = dir.path().join("raw.iso");
    // A real disc header, so an RVZ keeps it in its own header too.
    let mut head = disc(b"GDJEB2");
    head.resize(0x640000 + 12345, 0);
    for (i, byte) in head.iter_mut().enumerate().take(0x300000).skip(0x2000) {
        *byte = (i * 31 % 251) as u8;
    }
    head[0x500000..0x500100].fill(7);
    std::fs::write(&raw, &head).unwrap();

    let packed = dir.path().join(name);
    pack(format, &raw, &packed, &mut |_, _| {}).unwrap();
    assert_eq!(detect(&packed).unwrap(), format);
    let out = dir.path().join("out.iso");
    unpack(format, &packed, &out, &mut |_, _| {}).unwrap();
    let back = std::fs::read(&out).unwrap();
    assert_eq!(&back[..head.len()], &head[..]);
    assert!(back[head.len()..].iter().all(|&b| b == 0));
}

#[test]
fn rvz_round_trips() {
    roundtrip(Format::Rvz, "disc.rvz");
}

#[test]
fn ciso_round_trips() {
    roundtrip(Format::Ciso, "disc.ciso");
}

#[test]
fn plain_images_and_foreign_containers_are_classified() {
    let dir = tempfile::tempdir().unwrap();
    let plain = dir.path().join("a.iso");
    std::fs::write(&plain, disc(b"GDJEB2")).unwrap();
    assert_eq!(detect(&plain).unwrap(), Format::Raw);
    let wia = dir.path().join("a.wia");
    std::fs::write(&wia, b"WIA\x01rest").unwrap();
    assert!(matches!(detect(&wia), Err(Error::Container(_))));
}

#[test]
fn damaged_containers_are_refused_not_panicked_on() {
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes) in [
        ("short.rvz", b"RVZ\x01".to_vec()),
        ("short.ciso", b"CISO".to_vec()),
    ] {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        let format = detect(&path).unwrap();
        let out = dir.path().join("out.iso");
        assert!(unpack(format, &path, &out, &mut |_, _| {}).is_err());
    }
}
