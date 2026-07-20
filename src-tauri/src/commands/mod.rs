pub mod activity;
pub mod app;
pub mod codex;
pub mod data;
pub mod quest;

use tauri::State;

use crate::{
    dto::{CodexConnectionInput, CodexConnectionStatus},
    error::AppError,
    infrastructure::codex::TIMEOUT,
    state::AppState,
};

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
