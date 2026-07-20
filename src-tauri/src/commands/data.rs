use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use chrono::{Local, Utc};
use serde_json::{Value, json};
use sqlx::{Column, Row, SqliteConnection, SqlitePool, TypeInfo, sqlite::SqliteRow};
use tauri::State;
use uuid::Uuid;

use crate::{
    dto::{BackupResult, ExportResult},
    error::AppError,
    state::AppState,
};

const EXPORT_SCHEMA_VERSION: i64 = 2;

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
    create_private_directory(&directory)?;
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
    create_private_directory(&directory)?;
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
    create_private_directory(&directory)?;
    let path = directory.join(format!("levelog-{}.json", file_timestamp()));
    let document = export_document(state.db.pool(), &exported_at).await?;
    let text = serde_json::to_vec_pretty(&document)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    atomic_private_write(&path, &text)?;
    Ok(ExportResult {
        path: path.to_string_lossy().into_owned(),
        schema_version: EXPORT_SCHEMA_VERSION,
        exported_at,
    })
}

async fn backup_database(pool: &SqlitePool, path: &Path) -> Result<(), AppError> {
    // SQLite's backup command produces a consistent snapshot even while WAL is active.
    // Write beside the destination, sync, then atomically publish the completed snapshot.
    let directory = path
        .parent()
        .ok_or_else(|| AppError::Internal("バックアップ先ディレクトリを解決できません".into()))?;
    create_private_directory(directory)?;
    let temporary = directory.join(format!(".levelog-backup-{}.tmp", Uuid::new_v4()));
    let result = async {
        sqlx::query("VACUUM INTO ?")
            .bind(temporary.to_string_lossy().as_ref())
            .execute(pool)
            .await?;
        set_private_file_permissions(&temporary)?;
        OpenOptions::new().read(true).open(&temporary)?.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok::<(), AppError>(())
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result?;
    Ok(())
}

async fn export_document(pool: &SqlitePool, exported_at: &str) -> Result<Value, AppError> {
    let mut transaction = pool.begin().await?;
    let document = export_document_from(&mut transaction, exported_at).await?;
    transaction.commit().await?;
    Ok(document)
}

async fn export_document_from(
    connection: &mut SqliteConnection,
    exported_at: &str,
) -> Result<Value, AppError> {
    let settings = sqlx::query(
        "SELECT key, value_json, updated_at FROM app_settings WHERE key = 'profile' ORDER BY key",
    )
    .fetch_all(&mut *connection)
    .await?
    .into_iter()
    .map(setting_json)
    .collect::<Result<Vec<_>, _>>()?;
    let migrations = rows(
        &mut *connection,
        "SELECT version, description, success, installed_on FROM _sqlx_migrations ORDER BY version",
    )
    .await?;
    Ok(json!({
        "schemaVersion": EXPORT_SCHEMA_VERSION,
        "exportedAt": exported_at,
        "manifest": {
            "exportSchemaVersion": EXPORT_SCHEMA_VERSION,
            "format": "levelog.local-data-export",
            "containsSensitiveUserContent": true,
            "sensitiveContent": ["rawActivityCapture", "submittedPayload", "aiRawOutput", "profileContext", "interviewAnswers"],
            "excludedSecretClasses": ["codexConnection", "codexExecutablePath", "authenticationTokens", "futureSecretSettings"],
            "restoreSupported": false,
            "databaseMigrations": migrations,
        },
        "settings": settings,
        "profileRevisions": rows(&mut *connection, "SELECT id, schema_version, revision, profile_json, created_at, supersedes_id FROM user_profile_revisions ORDER BY revision").await?,
        "focusThemes": rows(&mut *connection, "SELECT id, title, desired_outcome, why_now, horizon, status, sort_order, created_at, updated_at FROM focus_themes ORDER BY sort_order, created_at, id").await?,
        "focusThemeSkillLinks": rows(&mut *connection, "SELECT theme_id, skill_id, relevance, created_at FROM focus_theme_skill_links ORDER BY theme_id, skill_id").await?,
        "activities": rows(&mut *connection, "SELECT id, occurred_on, action_text, challenge_text, outcome_text, created_at FROM activities ORDER BY created_at").await?,
        "activityCaptures": rows(&mut *connection, "SELECT id, activity_id, raw_text, capture_mode, created_at FROM activity_captures ORDER BY created_at").await?,
        "activityWorkflows": rows(&mut *connection, "SELECT activity_id, state, version, updated_at FROM activity_workflows ORDER BY updated_at, activity_id").await?,
        "activityStructures": rows(&mut *connection, "SELECT id, activity_id, analysis_id, revision, structured_json, source, prompt_version, schema_version, created_at FROM activity_structures ORDER BY activity_id, revision").await?,
        "interviewSessions": rows(&mut *connection, "SELECT id, activity_id, analysis_id, status, current_question_json, prompt_version, schema_version, created_at, updated_at FROM interview_sessions ORDER BY created_at").await?,
        "interviewAnswers": rows(&mut *connection, "SELECT id, session_id, question_id, answer_json, answer_state, created_at FROM interview_answers ORDER BY created_at, id").await?,
        "analyses": rows(&mut *connection, "SELECT id, activity_id, status, submitted_payload, raw_result_json, provider, model, codex_version, prompt_version, schema_version, error_message, created_at, completed_at, confirmed_at FROM ai_analyses ORDER BY created_at").await?,
        "candidates": rows(&mut *connection, "SELECT id, analysis_id, skill_id, specialized_skill_name, normalized_specialized_skill_name, confidence, reason, evidence, decision, edited_reason, edited_evidence, decided_at FROM skill_candidates ORDER BY analysis_id, id").await?,
        "observations": rows(&mut *connection, "SELECT id, activity_id, analysis_id, skill_id, specialized_skill_name, normalized_specialized_skill_name, evidence, source, created_at FROM skill_observations ORDER BY created_at").await?,
        "quests": rows(&mut *connection, "SELECT id, template_id, title, description, quest_type, status, target_skill_id, focus_theme_id, difficulty, estimated_minutes, success_criteria_json, evidence_prompt, scheduled_on, created_at, updated_at FROM quests ORDER BY created_at").await?,
        "questGenerationRuns": rows(&mut *connection, "SELECT id, activity_id, analysis_id, quest_id, status, submitted_payload, raw_result_json, provider, prompt_version, schema_version, error_message, created_at, completed_at FROM quest_generation_runs ORDER BY created_at, id").await?,
        "reflections": rows(&mut *connection, "SELECT id, quest_id, result, learned, difficulty_actual, next_action, created_at FROM quest_reflections ORDER BY created_at").await?,
        "xpEvents": rows(&mut *connection, "SELECT id, amount, reason_type, reason_key, activity_id, analysis_id, quest_id, description, created_at FROM xp_events ORDER BY created_at").await?,
    }))
}

async fn rows(connection: &mut SqliteConnection, sql: &str) -> Result<Vec<Value>, AppError> {
    sqlx::query(sql)
        .fetch_all(connection)
        .await?
        .iter()
        .map(row_json)
        .collect()
}

fn create_private_directory(path: &Path) -> Result<(), AppError> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn set_private_file_permissions(path: &Path) -> Result<(), AppError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn atomic_private_write(path: &Path, contents: &[u8]) -> Result<(), AppError> {
    let directory = path
        .parent()
        .ok_or_else(|| AppError::Internal("書き出し先ディレクトリを解決できません".into()))?;
    create_private_directory(directory)?;
    let temporary = directory.join(format!(".levelog-export-{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        set_private_file_permissions(&temporary)?;
        file.write_all(contents)?;
        file.flush()?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok::<(), AppError>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn setting_json(row: SqliteRow) -> Result<Value, AppError> {
    let mut value: Value = serde_json::from_str(row.get::<String, _>("value_json").as_str())
        .map_err(|error| AppError::Internal(error.to_string()))?;
    // The legacy profile was allowed to carry a local Codex path. It is not a
    // credential, but is local machine metadata and must not leave the device.
    if let Value::Object(object) = &mut value {
        object.remove("codexPath");
        object.remove("codex_path");
    }
    Ok(
        json!({ "key": row.get::<String, _>("key"), "value": value, "updatedAt": row.get::<String, _>("updated_at") }),
    )
}

fn row_json(row: &SqliteRow) -> Result<Value, AppError> {
    let mut object = serde_json::Map::new();
    for column in row.columns() {
        let key = column.name();
        let value = match column.type_info().name() {
            "INTEGER" | "BOOLEAN" => row
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
        sqlx::query("INSERT INTO app_settings (key, value_json, updated_at) VALUES ('profile', '{\"role\":\"developer\",\"codexPath\":\"/private/codex\"}', '2026-07-20T00:00:00Z'), ('codex_connection', '{\"path\":\"/private/codex\"}', '2026-07-20T00:00:00Z'), ('future_secret', '\"do-not-export\"', '2026-07-20T00:00:00Z')").execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO user_profile_revisions (id, schema_version, revision, profile_json, created_at) VALUES ('profile-1', 2, 1, '{\"role\":\"developer\"}', '2026-07-20T00:00:00Z')").execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO focus_themes (id, title, created_at, updated_at) VALUES ('theme-1', '品質', '2026-07-20T00:00:00Z', '2026-07-20T00:00:00Z')").execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO focus_theme_skill_links (theme_id, skill_id, relevance, created_at) VALUES ('theme-1', 'technical.validation', 1, '2026-07-20T00:00:00Z')").execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO activities (id, occurred_on, action_text, created_at) VALUES ('activity-1', '2026-07-20', 'legacy action', '2026-07-20T00:00:00Z')").execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO activity_captures (id, activity_id, raw_text, capture_mode, created_at) VALUES ('capture-1', 'activity-1', 'raw capture', 'quick', '2026-07-20T00:00:00Z')").execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO activity_workflows (activity_id, state, version, updated_at) VALUES ('activity-1', 'needs_input', 2, '2026-07-20T00:00:00Z')").execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO ai_analyses (id, activity_id, status, submitted_payload, provider, prompt_version, schema_version, created_at) VALUES ('analysis-1', 'activity-1', 'confirmed', '{\"safe\":true}', 'codex', 'v2', 'v2', '2026-07-20T00:00:00Z')").execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO activity_structures (id, activity_id, analysis_id, revision, structured_json, source, prompt_version, schema_version, created_at) VALUES ('structure-1', 'activity-1', 'analysis-1', 1, '{\"confirmedFacts\":[\"fact\"]}', 'codex_analysis', 'v2', 'v2', '2026-07-20T00:00:00Z')").execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO interview_sessions (id, activity_id, analysis_id, current_question_json, prompt_version, schema_version, created_at, updated_at) VALUES ('session-1', 'activity-1', 'analysis-1', '{\"questionId\":\"outcome\"}', 'v2', 'v2', '2026-07-20T00:00:00Z', '2026-07-20T00:00:00Z')").execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO interview_answers (id, session_id, question_id, answer_json, answer_state, created_at) VALUES ('answer-1', 'session-1', 'outcome', '{\"answer\":\"yes\"}', 'answered', '2026-07-20T00:00:00Z')").execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO quest_generation_runs (id, activity_id, analysis_id, status, submitted_payload, provider, prompt_version, schema_version, created_at) VALUES ('quest-run-1', 'activity-1', 'analysis-1', 'failed', '{\"edited\":true}', 'codex-cli', 'quest-proposal.v1', 'quest-proposal.v1', '2026-07-20T00:00:00Z')").execute(db.pool()).await.unwrap();
        let export = export_document(db.pool(), "2026-07-20T00:00:00Z")
            .await
            .unwrap();
        assert_eq!(export["schemaVersion"], EXPORT_SCHEMA_VERSION);
        assert_eq!(export["settings"].as_array().unwrap().len(), 1);
        assert_eq!(export["settings"][0]["value"]["codexPath"], Value::Null);
        assert_eq!(export["manifest"]["restoreSupported"], false);
        assert!(
            export["manifest"]["excludedSecretClasses"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == "codexConnection")
        );
        for key in [
            "activities",
            "analyses",
            "candidates",
            "observations",
            "quests",
            "reflections",
            "xpEvents",
            "profileRevisions",
            "focusThemes",
            "focusThemeSkillLinks",
            "activityCaptures",
            "activityWorkflows",
            "activityStructures",
            "interviewSessions",
            "interviewAnswers",
            "questGenerationRuns",
        ] {
            assert!(export.get(key).is_some());
        }
        assert_eq!(export["profileRevisions"].as_array().unwrap().len(), 1);
        assert_eq!(export["activityCaptures"][0]["raw_text"], "raw capture");
        assert_eq!(export["interviewAnswers"][0]["answer_state"], "answered");
        assert_eq!(
            export["questGenerationRuns"][0]["submitted_payload"],
            "{\"edited\":true}"
        );
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn json_export_is_atomically_written_with_private_permissions() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("export.json");
        atomic_private_write(&destination, br#"{"schemaVersion":2}"#).unwrap();
        assert_eq!(
            std::fs::read_to_string(&destination).unwrap(),
            r#"{"schemaVersion":2}"#
        );
        assert_eq!(
            std::fs::read_dir(directory.path())
                .unwrap()
                .filter_map(Result::ok)
                .count(),
            1
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&destination)
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
            assert_eq!(
                std::fs::metadata(directory.path())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
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
