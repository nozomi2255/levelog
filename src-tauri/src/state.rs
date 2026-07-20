use std::{collections::HashMap, path::PathBuf, sync::Arc};

use tokio::sync::{Mutex, Semaphore, watch};

use crate::{application::GrowthService, error::AppError, infrastructure::database::Database};

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub growth: GrowthService,
    pub app_data_dir: PathBuf,
    pub analysis_cancellations: Arc<Mutex<HashMap<String, watch::Sender<bool>>>>,
    pub codex_semaphore: Arc<Semaphore>,
}

impl AppState {
    pub async fn initialize(app_data_dir: PathBuf) -> Result<Self, AppError> {
        std::fs::create_dir_all(&app_data_dir)?;
        let db = Database::open(app_data_dir.join("levelog.db")).await?;
        sqlx::query("UPDATE ai_analyses SET status = 'failed', error_message = 'アプリ終了により解析が中断されました', completed_at = datetime('now') WHERE status = 'running'")
            .execute(db.pool())
            .await?;
        let growth = GrowthService::new(db.clone());
        Ok(Self {
            db,
            growth,
            app_data_dir,
            analysis_cancellations: Arc::new(Mutex::new(HashMap::new())),
            codex_semaphore: Arc::new(Semaphore::new(1)),
        })
    }
}
