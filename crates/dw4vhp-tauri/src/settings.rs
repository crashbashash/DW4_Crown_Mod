//! Portable settings contain patch options only, never disc or output paths.
use dw4vhp_core::options::PatchSelection;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;
use tauri_plugin_dialog::DialogExt;

const FORMAT: &str = "dw4-crown-mod-settings";
const VERSION: u32 = 1;
const MAX_BYTES: u64 = 64 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsFile {
    format: String,
    version: u32,
    settings: PatchSelection,
}

fn validate(settings: PatchSelection) -> Result<PatchSelection, String> {
    if settings.crown_drops && !settings.crown_mod {
        return Err("Match drops to crowns requires Crown Mod.".into());
    }
    if settings.random_bosses && !settings.random_enemies {
        return Err("Random bosses requires Random enemies.".into());
    }
    if settings.random_enemies && settings.enemy_seed.is_none() {
        return Err("Random enemies requires a valid seed.".into());
    }
    Ok(settings)
}

fn encode(settings: PatchSelection) -> Result<String, String> {
    let file = SettingsFile {
        format: FORMAT.into(),
        version: VERSION,
        settings: validate(settings)?,
    };
    serde_json::to_string_pretty(&file)
        .map(|json| json + "\n")
        .map_err(|error| format!("Unable to encode settings: {error}"))
}

fn read(path: &Path) -> Result<PatchSelection, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(MAX_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|_| "Unable to read the settings file.".to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("The settings file is too large.".into());
    }
    let file: SettingsFile = serde_json::from_slice(&bytes)
        .map_err(|_| "Invalid settings file. Choose a Crown Mod settings JSON file.".to_string())?;
    if file.format != FORMAT {
        return Err("This is not a Crown Mod settings file.".into());
    }
    if file.version != VERSION {
        return Err(
            "This settings file uses an unsupported version. Update Crown Mod and try again."
                .into(),
        );
    }
    validate(file.settings)
}

#[tauri::command]
pub async fn import_settings(window: tauri::Window) -> Result<Option<PatchSelection>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = window
            .dialog()
            .file()
            .set_parent(&window)
            .add_filter("Crown Mod settings", &["json"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let path = file
            .into_path()
            .map_err(|_| "Choose a local settings file.".to_string())?;
        read(&path).map(Some)
    })
    .await
    .map_err(|_| "Unable to open the settings dialog.".to_string())?
}

#[tauri::command]
pub async fn export_settings(
    window: tauri::Window,
    selection: PatchSelection,
) -> Result<bool, String> {
    let json = encode(selection)?;
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = window
            .dialog()
            .file()
            .set_parent(&window)
            .add_filter("Crown Mod settings", &["json"])
            .set_file_name("crown-mod-settings.json")
            .blocking_save_file()
        else {
            return Ok(false);
        };
        let path = file
            .into_path()
            .map_err(|_| "Choose a local settings file.".to_string())?;
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            return Err("Save settings with a .json extension.".into());
        }
        std::fs::write(path, json).map_err(|_| "Unable to save the settings file.".to_string())?;
        Ok(true)
    })
    .await
    .map_err(|_| "Unable to open the settings dialog.".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn settings_round_trip_preserves_all_options_and_seed_without_paths() {
        let selection = PatchSelection {
            crown_mod: true,
            crown_drops: true,
            four_player_spawns: true,
            random_crown_colours: true,
            random_enemies: true,
            random_bosses: true,
            enemy_seed: Some(u32::MAX),
            solo_buttons: true,
        };
        let encoded = encode(selection).unwrap();
        let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(
            value,
            json!({"format": FORMAT, "version": VERSION, "settings": selection})
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, encoded).unwrap();
        assert_eq!(read(&path).unwrap(), selection);
        assert!(encode(PatchSelection::default()).is_ok());
    }

    #[test]
    fn bad_imports_are_rejected_including_paths_and_invalid_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        for value in [
            json!({"format": FORMAT, "version": 2, "settings": {}}),
            json!({"format": "another-app", "version": 1, "settings": {}}),
            json!({"format": FORMAT, "version": 1, "settings": {"crownMod": "true"}}),
            json!({"format": FORMAT, "version": 1, "settings": {"enemySeed": 4294967296u64}}),
            json!({"format": FORMAT, "version": 1, "settings": {"randomEnemies": true}}),
            json!({"format": FORMAT, "version": 1, "settings": {"randomBosses": true}}),
            json!({"format": FORMAT, "version": 1, "settings": {"crownDrops": true}}),
            json!({"format": FORMAT, "version": 1, "settings": {"inputPath": "game.iso"}}),
            json!({"format": FORMAT, "version": 1, "settings": {}, "outputPath": "out.iso"}),
        ] {
            std::fs::write(&path, value.to_string()).unwrap();
            assert!(read(&path).is_err(), "{value}");
        }
        std::fs::write(&path, "not json").unwrap();
        assert!(read(&path).is_err());
        std::fs::write(&path, vec![b' '; MAX_BYTES as usize + 1]).unwrap();
        assert!(read(&path).unwrap_err().contains("too large"));
    }

    #[test]
    fn older_settings_leave_bosses_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(
            &path,
            json!({"format": FORMAT, "version": 1,
            "settings": {"randomEnemies": true, "enemySeed": 1520}})
            .to_string(),
        )
        .unwrap();
        let settings = read(&path).unwrap();
        assert!(settings.random_enemies);
        assert!(!settings.random_bosses);
    }
}
