#[cfg(feature = "agent-control")]
mod agent;
mod commands;
mod state;

pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::default());
    #[cfg(feature = "agent-control")]
    let builder = builder
        .manage(agent::AgentState::default())
        .setup(|app| agent::start(app.handle().clone()))
        .invoke_handler(tauri::generate_handler![
            commands::choose_iso,
            commands::choose_output,
            commands::analyze,
            commands::plan_summary,
            commands::start_patch,
            commands::default_output,
            commands::output_exists,
            agent::agent_reply,
        ]);
    #[cfg(not(feature = "agent-control"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::choose_iso,
        commands::choose_output,
        commands::analyze,
        commands::plan_summary,
        commands::start_patch,
        commands::default_output,
        commands::output_exists,
    ]);
    builder
        .run(tauri::generate_context!())
        .expect("error while running the DW4 Very Hard Plus window");
}
