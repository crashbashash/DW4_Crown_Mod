//! Settings file dialogs; the file format lives in `dw4vhp_core::settings`.
use dw4vhp_core::options::PatchSelection;
use dw4vhp_core::settings::{encode, read};
use tauri_plugin_dialog::DialogExt;

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
