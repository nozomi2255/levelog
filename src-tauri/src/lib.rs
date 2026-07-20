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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let state = tauri::async_runtime::block_on(state::AppState::initialize(app_data_dir))?;
            tauri::async_runtime::block_on(commands::data::create_daily_backup_if_needed(&state))?;
            app.manage(state);
            app.manage(commands::update::PendingAppUpdate::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_boot_state,
            commands::app::save_onboarding,
            commands::app::get_user_profile,
            commands::app::update_user_profile,
            commands::app::list_focus_themes,
            commands::app::save_focus_themes,
            commands::app::update_codex_path,
            commands::app::get_dashboard,
            commands::app::list_skills,
            commands::discover_codex_candidates,
            commands::test_codex_connection,
            commands::activity::create_activity,
            commands::activity::quick_capture_activity,
            commands::activity::list_activities,
            commands::activity::list_activity_inbox,
            commands::activity::get_activity,
            commands::activity::get_activity_workflow,
            commands::activity::get_analysis_preview,
            commands::activity::start_activity_analysis,
            commands::activity::get_activity_analysis,
            commands::activity::cancel_activity_analysis,
            commands::activity::answer_activity_question,
            commands::activity::confirm_activity_analysis,
            commands::quest::get_quest_preview,
            commands::quest::generate_quest,
            commands::quest::list_quests,
            commands::quest::transition_quest,
            commands::quest::save_quest_reflection,
            commands::evidence::import_pasted_source,
            commands::evidence_import::pick_and_import_sources,
            commands::evidence::list_evidence_library,
            commands::evidence::get_evidence_source,
            commands::evidence::create_evidence_claim,
            commands::evidence::review_evidence_claim,
            commands::evidence::link_claim_to_activity,
            commands::evidence::list_evidence_relations,
            commands::evidence::create_evidence_relation,
            commands::evidence::delete_evidence_relation,
            commands::evidence::create_project,
            commands::evidence::list_projects,
            commands::evidence::get_project,
            commands::evidence::link_claim_to_project,
            commands::evidence::unlink_claim_from_project,
            commands::evidence::create_portfolio_draft,
            commands::evidence::update_portfolio_draft,
            commands::evidence::list_portfolio_drafts,
            commands::evidence_import::get_evidence_analysis_preview,
            commands::evidence_import::start_evidence_analysis,
            commands::evidence_import::get_evidence_analysis,
            commands::evidence_import::cancel_evidence_analysis,
            commands::data::create_backup,
            commands::data::export_json,
            commands::update::get_release_info,
            commands::update::check_for_app_update,
            commands::update::install_app_update
        ])
        .run(tauri::generate_context!())
        .expect("error while running Levelog");
}
