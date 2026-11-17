//! The output name a disc image suggests.
use crate::container::{detect, Format};
use std::path::{Path, PathBuf};

/// The marker a patched image's name carries.
pub const OUTPUT_MARKER: &str = " [CrownMod]";

/// Same directory, stem plus the marker and optional enemy seed
/// (`game [CrownMod] [Seed 1234].iso`), so the default never targets the input.
/// Compressed containers keep their format; plain GameCube .gcm keeps its suffix too.
pub fn default_output(input: &Path, seed: Option<u32>) -> PathBuf {
    let mut name = input
        .file_stem()
        .map(|s| s.to_os_string())
        .unwrap_or_default();
    name.push(OUTPUT_MARKER);
    if let Some(seed) = seed {
        name.push(format!(" [Seed {seed}]"));
    }
    let format = detect(input).unwrap_or(Format::Raw);
    let extension = match format {
        Format::Raw
            if input
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("gcm")) =>
        {
            "gcm"
        }
        _ => format.extension(),
    };
    name.push(format!(".{extension}"));
    input.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_output_preserves_the_source_container_and_plain_gcm_suffix() {
        let dir = tempfile::tempdir().unwrap();
        for (name, magic, suffix) in [
            ("game.iso", b"raw!", "iso"),
            ("game.gcm", b"raw!", "gcm"),
            ("game.GCM", b"raw!", "gcm"),
            ("game.ciso", b"CISO", "ciso"),
            ("game.rvz", b"RVZ\x01", "rvz"),
            // The output suffix follows the bytes when an input is misnamed.
            ("compressed.iso", b"RVZ\x01", "rvz"),
        ] {
            let input = dir.path().join(name);
            std::fs::write(&input, magic).unwrap();
            let output = default_output(&input, None);
            assert_eq!(output.parent(), Some(dir.path()));
            assert_eq!(output.extension().unwrap(), suffix);
            assert!(output
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with(OUTPUT_MARKER));
            assert_ne!(output, input);
        }
    }

    #[test]
    fn suggested_output_names_the_enemy_seed_after_the_marker() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("game.iso");
        std::fs::write(&input, b"raw!").unwrap();
        assert_eq!(
            default_output(&input, Some(1234)),
            dir.path().join("game [CrownMod] [Seed 1234].iso")
        );
    }
}
