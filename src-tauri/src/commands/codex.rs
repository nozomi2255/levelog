//! Purpose-specific command helpers for Codex connectivity.
//!
//! The parent command module wires these helpers into Tauri commands; no arbitrary process
//! execution is exposed to the webview.

use crate::{
    dto::CodexConnectionStatus,
    infrastructure::codex::{CodexClient, TokioProcessRunner},
};
use std::path::PathBuf;

pub async fn test_connection(codex_path: String) -> CodexConnectionStatus {
    let path = PathBuf::from(codex_path);
    let display_path = path.display().to_string();
    match CodexClient::new(path, TokioProcessRunner) {
        Ok(client) => match client.probe().await {
            Ok(connection) => CodexConnectionStatus {
                available: true,
                authenticated: true,
                path: connection.path.display().to_string(),
                version: Some(connection.version),
                message: "Codex CLI is ready with required safety controls.".into(),
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
