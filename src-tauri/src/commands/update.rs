use std::{sync::Mutex, time::Duration};

use serde::Serialize;
use tauri::{AppHandle, State, Url, ipc::Channel};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::error::AppError;

const UPDATE_CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const UPDATE_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);
const UPDATE_ENDPOINT: Option<&str> = option_env!("LEVELOG_UPDATER_ENDPOINT");
const UPDATE_PUBLIC_KEY: Option<&str> = option_env!("LEVELOG_UPDATER_PUBLIC_KEY");

pub struct PendingAppUpdate(pub Mutex<Option<Update>>);

impl Default for PendingAppUpdate {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfoDto {
    pub current_version: String,
    pub updater_configured: bool,
    pub release_channel: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateDto {
    pub current_version: String,
    pub version: String,
    pub published_at: Option<String>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum AppUpdateEvent {
    Started { content_length: Option<u64> },
    Progress { chunk_length: usize },
    Finished,
    Installed,
}

fn release_configuration() -> Result<(Url, &'static str), AppError> {
    let endpoint = UPDATE_ENDPOINT
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let public_key = UPDATE_PUBLIC_KEY
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let (endpoint, public_key) = match (endpoint, public_key) {
        (Some(endpoint), Some(public_key)) => (endpoint, public_key),
        (None, None) => {
            return Err(AppError::InvalidState(
                "この開発ビルドには更新チャネルが設定されていません".into(),
            ));
        }
        _ => {
            return Err(AppError::Internal(
                "更新エンドポイントと署名公開鍵は両方設定する必要があります".into(),
            ));
        }
    };

    let endpoint = Url::parse(endpoint)
        .map_err(|_| AppError::Internal("更新エンドポイントが有効なURLではありません".into()))?;
    if endpoint.scheme() != "https" {
        return Err(AppError::Internal(
            "更新エンドポイントはHTTPSである必要があります".into(),
        ));
    }
    Ok((endpoint, public_key))
}

fn is_release_configured() -> bool {
    release_configuration().is_ok()
}

#[tauri::command]
pub fn get_release_info(app: AppHandle) -> ReleaseInfoDto {
    ReleaseInfoDto {
        current_version: app.package_info().version.to_string(),
        updater_configured: is_release_configured(),
        release_channel: "GitHub Releases / stable",
    }
}

#[tauri::command]
pub async fn check_for_app_update(
    app: AppHandle,
    state: State<'_, PendingAppUpdate>,
) -> Result<Option<AppUpdateDto>, AppError> {
    let (endpoint, public_key) = release_configuration()?;
    let updater = app
        .updater_builder()
        .endpoints(vec![endpoint])?
        .pubkey(public_key)
        .timeout(UPDATE_CHECK_TIMEOUT)
        .build()?;

    let mut update = updater.check().await?;
    if let Some(pending) = update.as_mut() {
        pending.timeout = Some(UPDATE_DOWNLOAD_TIMEOUT);
    }
    let dto = update.as_ref().map(|update| AppUpdateDto {
        current_version: update.current_version.clone(),
        version: update.version.clone(),
        published_at: update.date.map(|date| date.to_string()),
        notes: update.body.clone(),
    });
    let mut pending = state
        .0
        .lock()
        .map_err(|_| AppError::Internal("更新状態を読み取れませんでした".into()))?;
    *pending = update;
    Ok(dto)
}

#[tauri::command]
pub async fn install_app_update(
    app: AppHandle,
    state: State<'_, PendingAppUpdate>,
    on_event: Channel<AppUpdateEvent>,
) -> Result<(), AppError> {
    let update = state
        .0
        .lock()
        .map_err(|_| AppError::Internal("更新状態を読み取れませんでした".into()))?
        .take()
        .ok_or_else(|| AppError::InvalidState("先に最新バージョンを確認してください".into()))?;

    let progress_channel = on_event.clone();
    let finish_channel = on_event.clone();
    let mut started = false;
    update
        .download_and_install(
            move |chunk_length, content_length| {
                if !started {
                    let _ = progress_channel.send(AppUpdateEvent::Started { content_length });
                    started = true;
                }
                let _ = progress_channel.send(AppUpdateEvent::Progress { chunk_length });
            },
            move || {
                let _ = finish_channel.send(AppUpdateEvent::Finished);
            },
        )
        .await?;
    let _ = on_event.send(AppUpdateEvent::Installed);
    app.restart();
}
