pub mod application;
mod commands;
pub mod domain;
pub mod dto;
pub mod error;
pub mod infrastructure;
pub mod state;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let state = tauri::async_runtime::block_on(state::AppState::initialize(app_data_dir))?;
            tauri::async_runtime::block_on(commands::data::create_daily_backup_if_needed(&state))?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_boot_state,
            commands::app::save_onboarding,
            commands::app::update_codex_path,
            commands::app::get_dashboard,
            commands::app::list_skills,
            commands::test_codex_connection,
            commands::activity::create_activity,
            commands::activity::list_activities,
            commands::activity::get_activity,
            commands::activity::get_analysis_preview,
            commands::activity::start_activity_analysis,
            commands::activity::get_activity_analysis,
            commands::activity::cancel_activity_analysis,
            commands::activity::confirm_activity_analysis,
            commands::quest::generate_quest,
            commands::quest::list_quests,
            commands::quest::transition_quest,
            commands::quest::save_quest_reflection,
            commands::data::create_backup,
            commands::data::export_json
        ])
        .run(tauri::generate_context!())
        .expect("error while running Levelog");
}
