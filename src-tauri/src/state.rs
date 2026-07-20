use std::{collections::HashMap, path::PathBuf, sync::Arc};

use tokio::sync::{Mutex, Semaphore, watch};

use crate::{application::GrowthService, error::AppError, infrastructure::database::Database};

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub growth: GrowthService,
    pub app_data_dir: PathBuf,
    pub analysis_cancellations: Arc<Mutex<HashMap<String, watch::Sender<bool>>>>,
    pub evidence_analysis_cancellations: Arc<Mutex<HashMap<String, watch::Sender<bool>>>>,
    pub codex_semaphore: Arc<Semaphore>,
}

impl AppState {
    pub async fn initialize(app_data_dir: PathBuf) -> Result<Self, AppError> {
        std::fs::create_dir_all(&app_data_dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&app_data_dir, std::fs::Permissions::from_mode(0o700))?;
        }
        let database_path = app_data_dir.join("levelog.db");
        let db = Database::open(&database_path).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&database_path, std::fs::Permissions::from_mode(0o600))?;
        }
        sqlx::query("UPDATE ai_analyses SET status = 'failed', error_message = 'アプリ終了により解析が中断されました', completed_at = datetime('now') WHERE status IN ('pending', 'running')")
            .execute(db.pool())
            .await?;
        sqlx::query("UPDATE activity_workflows SET state = 'assessable', version = version + 1, updated_at = datetime('now') WHERE state = 'analysis_running' AND (SELECT status FROM ai_analyses WHERE activity_id = activity_workflows.activity_id ORDER BY created_at DESC, rowid DESC LIMIT 1) = 'failed'")
            .execute(db.pool())
            .await?;
        sqlx::query("UPDATE quest_generation_runs SET status = 'failed', error_message = 'アプリ終了によりクエスト生成が中断されました', completed_at = datetime('now') WHERE status IN ('pending', 'running')")
            .execute(db.pool())
            .await?;
        sqlx::query("UPDATE evidence_analysis_jobs SET status = 'failed', error_message = 'アプリ終了により解析が中断されました。原文は安全に保存されています', completed_at = datetime('now') WHERE status IN ('pending', 'running')")
            .execute(db.pool())
            .await?;
        let growth = GrowthService::new(db.clone());
        Ok(Self {
            db,
            growth,
            app_data_dir,
            analysis_cancellations: Arc::new(Mutex::new(HashMap::new())),
            evidence_analysis_cancellations: Arc::new(Mutex::new(HashMap::new())),
            codex_semaphore: Arc::new(Semaphore::new(1)),
        })
    }
}
