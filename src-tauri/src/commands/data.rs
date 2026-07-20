use std::path::Path;

use chrono::{Local, Utc};
use serde_json::{Value, json};
use sqlx::{Column, Row, SqlitePool, TypeInfo, sqlite::SqliteRow};
use tauri::State;

use crate::{
    dto::{BackupResult, ExportResult},
    error::AppError,
    state::AppState,
};

const EXPORT_SCHEMA_VERSION: i64 = 1;

/// Creates at most one consistent backup per local calendar day. The marker is
/// written only after the database snapshot is safely available.
pub async fn create_daily_backup_if_needed(state: &AppState) -> Result<(), AppError> {
    let today = Local::now().date_naive().to_string();
    let previous_raw: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE key = 'last_daily_backup'")
            .fetch_optional(state.db.pool())
            .await?;
    let previous = previous_raw.as_deref().and_then(|value| {
        serde_json::from_str::<String>(value)
            .ok()
            .or_else(|| Some(value.to_owned()))
    });
    if previous.as_deref() == Some(today.as_str()) {
        return Ok(());
    }
    let directory = state.app_data_dir.join("backups");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(format!("levelog-daily-{today}.db"));
    if !path.exists() {
        backup_database(state.db.pool(), &path).await?;
    }
    sqlx::query("INSERT INTO app_settings (key, value_json, updated_at) VALUES ('last_daily_backup', ?, ?) ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at")
        .bind(serde_json::to_string(&today).map_err(|error| AppError::Internal(error.to_string()))?)
        .bind(timestamp())
        .execute(state.db.pool())
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn create_backup(state: State<'_, AppState>) -> Result<BackupResult, AppError> {
    let created_at = timestamp();
    let directory = state.app_data_dir.join("backups");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(format!("levelog-{}.db", file_timestamp()));
    backup_database(state.db.pool(), &path).await?;
    Ok(BackupResult {
        path: path.to_string_lossy().into_owned(),
        created_at,
    })
}

#[tauri::command]
pub async fn export_json(state: State<'_, AppState>) -> Result<ExportResult, AppError> {
    let exported_at = timestamp();
    let directory = state.app_data_dir.join("exports");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(format!("levelog-{}.json", file_timestamp()));
    let document = export_document(state.db.pool(), &exported_at).await?;
    let text = serde_json::to_vec_pretty(&document)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    std::fs::write(&path, text)?;
    Ok(ExportResult {
        path: path.to_string_lossy().into_owned(),
        schema_version: EXPORT_SCHEMA_VERSION,
        exported_at,
    })
}

async fn backup_database(pool: &SqlitePool, path: &Path) -> Result<(), AppError> {
    // SQLite's backup command produces a consistent snapshot even while WAL is active.
    sqlx::query("VACUUM INTO ?")
        .bind(path.to_string_lossy().as_ref())
        .execute(pool)
        .await?;
    Ok(())
}

async fn export_document(pool: &SqlitePool, exported_at: &str) -> Result<Value, AppError> {
    let settings = sqlx::query(
        "SELECT key, value_json, updated_at FROM app_settings WHERE key = 'profile' ORDER BY key",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(setting_json)
    .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "schemaVersion": EXPORT_SCHEMA_VERSION,
        "exportedAt": exported_at,
        "settings": settings,
        "activities": rows(pool, "SELECT id, occurred_on, action_text, challenge_text, outcome_text, created_at FROM activities ORDER BY created_at").await?,
        "analyses": rows(pool, "SELECT id, activity_id, status, submitted_payload, raw_result_json, provider, model, codex_version, prompt_version, schema_version, error_message, created_at, completed_at, confirmed_at FROM ai_analyses ORDER BY created_at").await?,
        "candidates": rows(pool, "SELECT id, analysis_id, skill_id, confidence, reason, evidence, decision, edited_reason, edited_evidence, decided_at FROM skill_candidates ORDER BY analysis_id, id").await?,
        "observations": rows(pool, "SELECT id, activity_id, analysis_id, skill_id, evidence, source, created_at FROM skill_observations ORDER BY created_at").await?,
        "quests": rows(pool, "SELECT id, template_id, title, description, quest_type, status, target_skill_id, difficulty, estimated_minutes, success_criteria_json, evidence_prompt, scheduled_on, created_at, updated_at FROM quests ORDER BY created_at").await?,
        "reflections": rows(pool, "SELECT id, quest_id, result, learned, difficulty_actual, next_action, created_at FROM quest_reflections ORDER BY created_at").await?,
        "xpEvents": rows(pool, "SELECT id, amount, reason_type, reason_key, activity_id, analysis_id, quest_id, description, created_at FROM xp_events ORDER BY created_at").await?,
    }))
}

async fn rows(pool: &SqlitePool, sql: &str) -> Result<Vec<Value>, AppError> {
    sqlx::query(sql)
        .fetch_all(pool)
        .await?
        .iter()
        .map(row_json)
        .collect()
}

fn setting_json(row: SqliteRow) -> Result<Value, AppError> {
    let value: Value = serde_json::from_str(row.get::<String, _>("value_json").as_str())
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(
        json!({ "key": row.get::<String, _>("key"), "value": value, "updatedAt": row.get::<String, _>("updated_at") }),
    )
}

fn row_json(row: &SqliteRow) -> Result<Value, AppError> {
    let mut object = serde_json::Map::new();
    for column in row.columns() {
        let key = column.name();
        let value = match column.type_info().name() {
            "INTEGER" => row
                .try_get::<Option<i64>, _>(key)
                .map(|value| value.map_or(Value::Null, Value::from)),
            "REAL" => row
                .try_get::<Option<f64>, _>(key)
                .map(|value| value.map_or(Value::Null, |number| json!(number))),
            _ => row
                .try_get::<Option<String>, _>(key)
                .map(|value| value.map_or(Value::Null, Value::from)),
        }
        .map_err(|error| AppError::Database(error.to_string()))?;
        object.insert(key.to_string(), value);
    }
    Ok(Value::Object(object))
}

fn timestamp() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
fn file_timestamp() -> String {
    Utc::now().format("%Y%m%d-%H%M%S-%3f").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::Database;

    #[tokio::test]
    async fn export_only_includes_allowlisted_settings_and_all_required_keys() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let db = Database::open(file.path()).await.unwrap();
        sqlx::query("INSERT INTO app_settings (key, value_json, updated_at) VALUES ('profile', '{\"role\":\"developer\"}', '2026-07-20T00:00:00Z'), ('future_secret', '\"do-not-export\"', '2026-07-20T00:00:00Z')").execute(db.pool()).await.unwrap();
        let export = export_document(db.pool(), "2026-07-20T00:00:00Z")
            .await
            .unwrap();
        assert_eq!(export["schemaVersion"], EXPORT_SCHEMA_VERSION);
        assert_eq!(export["settings"].as_array().unwrap().len(), 1);
        for key in [
            "activities",
            "analyses",
            "candidates",
            "observations",
            "quests",
            "reflections",
            "xpEvents",
        ] {
            assert!(export.get(key).is_some());
        }
    }

    #[tokio::test]
    async fn vacuum_backup_creates_a_readable_database() {
        let source = tempfile::NamedTempFile::new().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let db = Database::open(source.path()).await.unwrap();
        let backup = destination.path().join("copy.db");
        backup_database(db.pool(), &backup).await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM skills")
            .fetch_one(
                &sqlx::SqlitePool::connect(&format!("sqlite:{}", backup.display()))
                    .await
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(count, 15);
    }

    #[tokio::test]
    async fn daily_backup_skips_when_the_local_date_is_already_recorded() {
        let directory = tempfile::tempdir().unwrap();
        let state = crate::state::AppState::initialize(directory.path().to_path_buf())
            .await
            .unwrap();
        create_daily_backup_if_needed(&state).await.unwrap();
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM app_settings WHERE key = 'last_daily_backup'")
                .fetch_one(state.db.pool())
                .await
                .unwrap();
        assert_eq!(count, 1);
        create_daily_backup_if_needed(&state).await.unwrap();
        let backups = std::fs::read_dir(directory.path().join("backups"))
            .unwrap()
            .count();
        assert_eq!(backups, 1);
    }
}
