//! Purpose-specific command helpers for Codex connectivity.
//!
//! The parent command module wires these helpers into Tauri commands; no arbitrary process
//! execution is exposed to the webview.

use crate::{
    dto::CodexConnectionStatus,
    infrastructure::codex::{CodexClient, TokioProcessRunner, discovery::validate_executable},
};
use std::path::PathBuf;

pub async fn test_connection(codex_path: String) -> CodexConnectionStatus {
    let discovered_path = PathBuf::from(codex_path);
    let executable = match validate_executable(&discovered_path) {
        Ok(executable) => executable,
        Err(message) => {
            return CodexConnectionStatus {
                available: false,
                authenticated: false,
                path: discovered_path.display().to_string(),
                version: None,
                message,
            };
        }
    };
    let display_path = executable.launch_path.display().to_string();
    match CodexClient::new(executable.launch_path, TokioProcessRunner) {
        Ok(client) => match client.probe().await {
            Ok(connection) => CodexConnectionStatus {
                available: true,
                authenticated: true,
                path: connection.path.display().to_string(),
                version: Some(connection.version),
                message: "Codex CLIは必要な安全制御を有効にして利用できます。".into(),
            },
            Err(error) => CodexConnectionStatus {
                available: !matches!(error, crate::infrastructure::codex::CodexError::NotFound(_)),
                authenticated: false,
                path: display_path,
                version: None,
                message: error.to_string(),
            },
        },
        Err(error) => CodexConnectionStatus {
            available: false,
            authenticated: false,
            path: display_path,
            version: None,
            message: error.to_string(),
        },
    }
}
