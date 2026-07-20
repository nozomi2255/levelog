pub mod activity;
pub mod app;
pub mod codex;
pub mod data;
pub mod evidence;
pub mod evidence_import;
pub mod quest;
pub mod update;

use tauri::State;

use crate::{
    dto::{CodexConnectionInput, CodexConnectionStatus, CodexPathCandidateDto, OnboardingInput},
    error::AppError,
    infrastructure::codex::TIMEOUT,
    state::AppState,
};

#[tauri::command]
pub async fn discover_codex_candidates(
    state: State<'_, AppState>,
) -> Result<Vec<CodexPathCandidateDto>, AppError> {
    let connection_json: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE key = 'codex_connection'")
            .fetch_optional(state.db.pool())
            .await?;
    let configured = connection_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<CodexConnectionStatus>(value).ok())
        .map(|value| value.path);
    let configured = if configured.is_some() {
        configured
    } else {
        let legacy_json: Option<String> =
            sqlx::query_scalar("SELECT value_json FROM app_settings WHERE key = 'profile'")
                .fetch_optional(state.db.pool())
                .await?;
        legacy_json
            .as_deref()
            .and_then(|value| serde_json::from_str::<OnboardingInput>(value).ok())
            .map(|value| value.codex_path)
    };
    Ok(crate::infrastructure::codex::discovery::discover(
        configured.as_deref(),
    ))
}

#[tauri::command]
pub async fn test_codex_connection(
    state: State<'_, AppState>,
    input: CodexConnectionInput,
) -> Result<CodexConnectionStatus, AppError> {
    let path = input.codex_path;
    let Ok(_permit) = state.codex_semaphore.clone().acquire_owned().await else {
        return Ok(CodexConnectionStatus {
            available: false,
            authenticated: false,
            path,
            version: None,
            message: "Codex実行キューを開始できませんでした".into(),
        });
    };
    match tokio::time::timeout(TIMEOUT, codex::test_connection(path.clone())).await {
        Ok(status) => Ok(status),
        Err(_) => Ok(CodexConnectionStatus {
            available: false,
            authenticated: false,
            path,
            version: None,
            message: "Codex接続確認が180秒でタイムアウトしました".into(),
        }),
    }
}
