use std::path::PathBuf;

use chrono::Local;
use serde_json::{Value, json};
use sqlx::{Column, Row, SqlitePool, TypeInfo};
use tauri::State;
use tokio::sync::watch;
use uuid::Uuid;

use crate::{
    application::NewQuest,
    domain::{QuestStatus, ReflectionResult},
    dto::{
        GenerateQuestInput, QuestDto, QuestReflectionDto, QuestReflectionInput,
        QuestTransitionInput, SubmissionPreview,
    },
    error::AppError,
    infrastructure::codex::{
        CodexClient, CodexError, QUEST_SCHEMA_VERSION, TIMEOUT, TokioProcessRunner,
    },
    state::AppState,
};

use super::app::quest_from_row;

const QUEST_PAYLOAD_SCHEMA_VERSION: &str = "quest-generation.v2";
const MAX_SUBMITTED_PAYLOAD_BYTES: usize = 64 * 1024;
const QUEST_CLOUD_NOTICE: &str = "このJSONだけがCodex CLIを通じてクラウド推論へ送信されます。活動の原文、未確認のAI推測、却下した候補は含めません。";

#[tauri::command]
pub async fn get_quest_preview(
    state: State<'_, AppState>,
    input: GenerateQuestInput,
) -> Result<SubmissionPreview, AppError> {
    let payload = quest_payload(&state, &input.activity_id, &input.analysis_id).await?;
    Ok(SubmissionPreview {
        entity_id: input.analysis_id,
        submitted_payload: serde_json::to_string_pretty(&payload)
            .map_err(|error| AppError::Internal(error.to_string()))?,
        cloud_inference_notice: QUEST_CLOUD_NOTICE.into(),
    })
}

#[tauri::command]
pub async fn generate_quest(
    state: State<'_, AppState>,
    input: GenerateQuestInput,
) -> Result<QuestDto, AppError> {
    let expected_payload = quest_payload(&state, &input.activity_id, &input.analysis_id).await?;
    let serialized_payload = match input.submitted_payload.as_deref() {
        Some(payload) => validate_submitted_payload(payload)?,
        None => serde_json::to_string_pretty(&expected_payload)
            .map_err(|error| AppError::Internal(error.to_string()))?,
    };
    let codex_path = codex_path(&state).await?;
    let client = CodexClient::new(PathBuf::from(codex_path), TokioProcessRunner)?;
    let run_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO quest_generation_runs (id, activity_id, analysis_id, status, submitted_payload, provider, prompt_version, schema_version, created_at) VALUES (?, ?, ?, 'running', ?, 'codex-cli', 'quest-proposal.v1', ?, ?)")
        .bind(&run_id)
        .bind(&input.activity_id)
        .bind(&input.analysis_id)
        .bind(&serialized_payload)
        .bind(QUEST_SCHEMA_VERSION)
        .bind(super::app::now())
        .execute(state.db.pool())
        .await?;
    let _permit = match state.codex_semaphore.clone().acquire_owned().await {
        Ok(permit) => permit,
        Err(_) => {
            let message = "Codex実行キューを開始できませんでした";
            mark_quest_run_failed(&state, &run_id, message, None).await?;
            return Err(AppError::Internal(message.into()));
        }
    };
    let (_cancel_sender, cancel_receiver) = watch::channel(false);
    let execution = async {
        let mut attempt = 0;
        loop {
            let result = client
                .propose_quest(serialized_payload.clone(), cancel_receiver.clone())
                .await;
            let retryable = result.as_ref().is_err_and(CodexError::is_schema_retryable);
            if retryable && attempt == 0 {
                attempt += 1;
                continue;
            }
            break result;
        }
    };
    let proposal_result = match tokio::time::timeout(TIMEOUT, execution).await {
        Ok(Ok(proposal)) => proposal,
        Ok(Err(error)) => {
            mark_quest_run_failed(&state, &run_id, &error.to_string(), error.raw_output()).await?;
            return Err(error.into());
        }
        Err(_) => {
            let error = CodexError::TimedOut;
            mark_quest_run_failed(&state, &run_id, &error.to_string(), None).await?;
            return Err(error.into());
        }
    };
    let raw_result_json = proposal_result.raw_json;
    let proposal = proposal_result.parsed;
    let id = match state
        .growth
        .create_quest_from_generation(
            NewQuest {
                template_id: proposal.template_id,
                title: proposal.title,
                description: proposal.description,
                target_skill_id: Some(proposal.target_skill_id),
                difficulty: proposal.difficulty,
                estimated_minutes: proposal.estimated_minutes,
                success_criteria_json: serde_json::to_string(&proposal.success_criteria)
                    .map_err(|error| AppError::Internal(error.to_string()))?,
                evidence_prompt: proposal.evidence_prompt,
                scheduled_on: Some(Local::now().date_naive().to_string()),
            },
            &run_id,
            &raw_result_json,
        )
        .await
    {
        Ok(id) => id,
        Err(error) => {
            mark_quest_run_failed(&state, &run_id, &error.to_string(), Some(&raw_result_json))
                .await?;
            return Err(error.into());
        }
    };
    quest_by_id(&state, &id).await
}

async fn mark_quest_run_failed(
    state: &AppState,
    run_id: &str,
    message: &str,
    raw_output: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query("UPDATE quest_generation_runs SET status = 'failed', error_message = ?, raw_result_json = COALESCE(?, raw_result_json), completed_at = ? WHERE id = ? AND status = 'running'")
        .bind(message)
        .bind(raw_output)
        .bind(super::app::now())
        .bind(run_id)
        .execute(state.db.pool())
        .await?;
    Ok(())
}

async fn codex_path(state: &AppState) -> Result<String, AppError> {
    let configured: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE key = 'codex_connection'")
            .fetch_optional(state.db.pool())
            .await?;
    let connection = configured
        .as_deref()
        .ok_or_else(|| AppError::InvalidState("Codex CLIを設定してください".into()))
        .and_then(|json| {
            serde_json::from_str::<crate::dto::CodexConnectionStatus>(json)
                .map_err(|error| AppError::Internal(error.to_string()))
        })?;
    if !connection.available || !connection.authenticated || connection.path.trim().is_empty() {
        return Err(AppError::InvalidState(
            "Codex CLIの接続を確認してください".into(),
        ));
    }
    Ok(connection.path)
}

fn validate_submitted_payload(payload: &str) -> Result<String, AppError> {
    if payload.trim().is_empty() {
        return Err(AppError::Validation("送信ペイロードが空です".into()));
    }
    if payload.len() > MAX_SUBMITTED_PAYLOAD_BYTES {
        return Err(AppError::Validation("送信ペイロードが大きすぎます".into()));
    }
    let submitted: Value = serde_json::from_str(payload)
        .map_err(|_| AppError::Validation("送信ペイロードは正しいJSONではありません".into()))?;
    if !submitted.is_object() {
        return Err(AppError::Validation(
            "送信ペイロードはJSONオブジェクトである必要があります".into(),
        ));
    }
    Ok(payload.to_owned())
}

async fn quest_payload(
    state: &AppState,
    activity_id: &str,
    analysis_id: &str,
) -> Result<Value, AppError> {
    let analysis = sqlx::query(
        "SELECT raw_result_json FROM ai_analyses WHERE id = ? AND activity_id = ? AND status = 'confirmed'",
    )
    .bind(analysis_id)
    .bind(activity_id)
    .fetch_optional(state.db.pool())
    .await?
    .ok_or_else(|| AppError::InvalidState("確認済みの分析からのみクエストを作成できます".into()))?;
    let raw_result: Option<String> = analysis.get("raw_result_json");
    let parsed = raw_result
        .as_deref()
        .ok_or_else(|| AppError::InvalidState("確認済み分析の構造化結果がありません".into()))
        .and_then(|raw| {
            crate::dto::parse_activity_analysis_output_compat(raw)
                .map_err(|_| AppError::InvalidState("確認済み分析の構造化結果を読めません".into()))
        })?;
    let profile_json: String = sqlx::query_scalar(
        "SELECT profile_json FROM user_profile_revisions ORDER BY revision DESC LIMIT 1",
    )
    .fetch_optional(state.db.pool())
    .await?
    .ok_or_else(|| AppError::InvalidState("成長プロフィールを設定してください".into()))?;
    let profile: Value = serde_json::from_str(&profile_json)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let active_themes = rows_as_json(state.db.pool(), "SELECT t.id, t.title, t.desired_outcome, t.why_now, t.horizon, t.sort_order, COALESCE(json_group_array(l.skill_id) FILTER (WHERE l.skill_id IS NOT NULL), json('[]')) AS linked_skill_ids FROM focus_themes t LEFT JOIN focus_theme_skill_links l ON l.theme_id = t.id WHERE t.status = 'active' GROUP BY t.id ORDER BY t.sort_order, t.created_at, t.id").await?;
    let evidence = rows_as_json_with(state.db.pool(), "SELECT skill_id, specialized_skill_name, evidence, created_at FROM skill_observations WHERE activity_id = ? AND analysis_id = ? AND source = 'analysis_confirmation' ORDER BY created_at, id", &[activity_id, analysis_id]).await?;
    Ok(json!({
        "schemaVersion": QUEST_PAYLOAD_SCHEMA_VERSION,
        "profile": {
            "role": profile.get("role").cloned().unwrap_or(Value::Null),
            "background": profile.get("background").cloned().unwrap_or(Value::Null),
            "currentResponsibilities": profile.get("currentResponsibilities").cloned().unwrap_or(Value::Null),
            "domainsAndTechnologies": profile.get("domainsAndTechnologies").cloned().unwrap_or_else(|| json!([])),
            "growthGoal": profile.get("growthGoal").cloned().unwrap_or(Value::Null),
            "motivation": profile.get("motivation").cloned().unwrap_or(Value::Null),
            "currentChallenges": profile.get("currentChallenges").cloned().unwrap_or(Value::Null),
            "recentSuccess": profile.get("recentSuccess").cloned().unwrap_or(Value::Null),
            "focusSkillIds": profile.get("focusSkillIds").cloned().unwrap_or_else(|| json!([])),
        },
        "focusThemes": active_themes,
        "questPreferences": {
            "weeklyMinutes": profile.get("weeklyMinutes").cloned().unwrap_or(Value::Null),
            "preferredQuestMinutes": profile.get("preferredQuestMinutes").cloned().unwrap_or(Value::Null),
            "preferredQuestStyle": profile.get("preferredQuestStyle").cloned().unwrap_or(Value::Null),
            "constraints": profile.get("constraints").cloned().unwrap_or(Value::Null),
            "excludedQuestPatterns": profile.get("excludedQuestPatterns").cloned().unwrap_or(Value::Null),
        },
        "confirmedAnalysis": {
            "confirmedFacts": parsed.confirmed_facts,
            "outcomes": parsed.outcomes,
            "confirmedEvidence": evidence,
        }
    }))
}

async fn rows_as_json(pool: &SqlitePool, sql: &str) -> Result<Vec<Value>, AppError> {
    rows_as_json_with(pool, sql, &[]).await
}

async fn rows_as_json_with(
    pool: &SqlitePool,
    sql: &str,
    bindings: &[&str],
) -> Result<Vec<Value>, AppError> {
    let mut query = sqlx::query(sql);
    for binding in bindings {
        query = query.bind(*binding);
    }
    query
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row| {
            let mut object = serde_json::Map::new();
            for column in row.columns() {
                let name = column.name();
                let value = if name == "linked_skill_ids" {
                    let raw: Option<String> = row
                        .try_get(name)
                        .map_err(|error| AppError::Database(error.to_string()))?;
                    raw.as_deref()
                        .map(|raw| serde_json::from_str(raw).unwrap_or_else(|_| json!([])))
                        .unwrap_or_else(|| json!([]))
                } else if matches!(column.type_info().name(), "INTEGER" | "BOOLEAN") {
                    row.try_get::<Option<i64>, _>(name)
                        .map_err(|error| AppError::Database(error.to_string()))?
                        .map_or(Value::Null, Value::from)
                } else if column.type_info().name() == "REAL" {
                    row.try_get::<Option<f64>, _>(name)
                        .map_err(|error| AppError::Database(error.to_string()))?
                        .map_or(Value::Null, |number| json!(number))
                } else {
                    let raw: Option<String> = row
                        .try_get(name)
                        .map_err(|error| AppError::Database(error.to_string()))?;
                    raw.map(Value::String).unwrap_or(Value::Null)
                };
                object.insert(name.to_owned(), value);
            }
            Ok(Value::Object(object))
        })
        .collect()
}

#[tauri::command]
pub async fn list_quests(state: State<'_, AppState>) -> Result<Vec<QuestDto>, AppError> {
    let rows = sqlx::query("SELECT * FROM quests ORDER BY updated_at DESC")
        .fetch_all(state.db.pool())
        .await?;
    Ok(rows.iter().map(quest_from_row).collect())
}

#[tauri::command]
pub async fn transition_quest(
    state: State<'_, AppState>,
    input: QuestTransitionInput,
) -> Result<QuestDto, AppError> {
    let next = match input.action.as_str() {
        "accept" => QuestStatus::Accepted,
        "start" => QuestStatus::InProgress,
        "complete" => QuestStatus::Completed,
        "reschedule" => QuestStatus::Rescheduled,
        "adjust" => QuestStatus::Adjusted,
        "cancel" => QuestStatus::Cancelled,
        _ => {
            return Err(AppError::Validation(format!(
                "未知のクエスト操作です: {}",
                input.action
            )));
        }
    };
    let scheduled_on = if next == QuestStatus::Rescheduled {
        Some(
            input
                .scheduled_on
                .ok_or_else(|| AppError::Validation("延期する日付を入力してください".into()))?,
        )
    } else {
        None
    };
    let estimated_minutes = if next == QuestStatus::Adjusted {
        let value = input
            .estimated_minutes
            .ok_or_else(|| AppError::Validation("縮小後の所要時間を入力してください".into()))?;
        if !(5..=30).contains(&value) {
            return Err(AppError::Validation(
                "所要時間は5〜30分で指定してください".into(),
            ));
        }
        Some(value)
    } else {
        None
    };
    state
        .growth
        .transition_quest_with_details(
            &input.quest_id,
            next,
            scheduled_on.as_deref(),
            estimated_minutes,
        )
        .await?;
    quest_by_id(&state, &input.quest_id).await
}

#[tauri::command]
pub async fn save_quest_reflection(
    state: State<'_, AppState>,
    input: QuestReflectionInput,
) -> Result<QuestReflectionDto, AppError> {
    if let Some(value) = input.difficulty_actual
        && !(1..=5).contains(&value)
    {
        return Err(AppError::Validation(
            "実際の難易度は1〜5で指定してください".into(),
        ));
    }
    let result = match input.result.as_str() {
        "completed" => ReflectionResult::Completed,
        "partially_completed" => ReflectionResult::PartiallyCompleted,
        "not_completed" => ReflectionResult::NotCompleted,
        "rested" => ReflectionResult::Rested,
        _ => {
            return Err(AppError::Validation(
                "振り返り結果が正しくありません".into(),
            ));
        }
    };
    let existed: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM quest_reflections WHERE quest_id = ?")
            .bind(&input.quest_id)
            .fetch_one(state.db.pool())
            .await?;
    state
        .growth
        .save_reflection(
            &input.quest_id,
            result,
            input.learned.trim(),
            input.difficulty_actual,
            input.next_action.trim(),
        )
        .await?;
    let row = sqlx::query("SELECT * FROM quest_reflections WHERE quest_id = ?")
        .bind(&input.quest_id)
        .fetch_one(state.db.pool())
        .await?;
    Ok(QuestReflectionDto {
        id: row.get("id"),
        quest_id: row.get("quest_id"),
        result: row.get("result"),
        learned: row.get("learned"),
        difficulty_actual: row.get("difficulty_actual"),
        next_action: row.get("next_action"),
        created_at: row.get("created_at"),
        xp_awarded: if existed == 0 { 40 } else { 0 },
    })
}

async fn quest_by_id(state: &AppState, id: &str) -> Result<QuestDto, AppError> {
    let row = sqlx::query("SELECT * FROM quests WHERE id = ?")
        .bind(id)
        .fetch_optional(state.db.pool())
        .await?
        .ok_or_else(|| AppError::NotFound("quest".into()))?;
    Ok(quest_from_row(&row))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn prepared_state() -> crate::state::AppState {
        let directory = tempfile::tempdir().unwrap().keep();
        let state = crate::state::AppState::initialize(directory).await.unwrap();
        let pool = state.db.pool();
        sqlx::query("INSERT INTO user_profile_revisions (id, schema_version, revision, profile_json, created_at) VALUES ('profile-1', 2, 1, ?, '2026-07-20T00:00:00Z')")
            .bind(r#"{"role":"developer","background":"backend","currentResponsibilities":"quality","domainsAndTechnologies":["Rust"],"growthGoal":"safer releases","motivation":"learn","currentChallenges":"review","recentSuccess":"tests","focusSkillIds":["technical.validation"],"weeklyMinutes":60,"preferredQuestMinutes":15,"preferredQuestStyle":"balanced","constraints":"no weekends","excludedQuestPatterns":"none"}"#)
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO focus_themes (id, title, desired_outcome, why_now, horizon, status, sort_order, created_at, updated_at) VALUES ('active-theme', '品質', '安全なリリース', '今必要', 'quarter', 'active', 0, '2026-07-20T00:00:00Z', '2026-07-20T00:00:00Z'), ('paused-theme', '休止', '', '', 'ongoing', 'paused', 1, '2026-07-20T00:00:00Z', '2026-07-20T00:00:00Z')")
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO focus_theme_skill_links (theme_id, skill_id, relevance, created_at) VALUES ('active-theme', 'technical.validation', 1, '2026-07-20T00:00:00Z')").execute(pool).await.unwrap();
        sqlx::query("INSERT INTO activities (id, occurred_on, action_text, challenge_text, outcome_text, created_at) VALUES ('activity-1', '2026-07-20', 'RAW ACTIVITY MUST NOT LEAK', '', '', '2026-07-20T00:00:00Z')").execute(pool).await.unwrap();
        sqlx::query("INSERT INTO ai_analyses (id, activity_id, status, submitted_payload, raw_result_json, provider, prompt_version, schema_version, created_at) VALUES ('analysis-1', 'activity-1', 'confirmed', '{}', ?, 'codex', 'v2', 'v2', '2026-07-20T00:00:00Z'), ('analysis-unconfirmed', 'activity-1', 'succeeded', '{}', ?, 'codex', 'v2', 'v2', '2026-07-20T00:00:00Z')")
            .bind(r#"{"summary":"AI assumption","outcomes":["confirmed outcome"],"confirmedFacts":["confirmed fact"],"unconfirmedFacts":["UNCONFIRMED MUST NOT LEAK"],"skillCandidates":[],"nextQuestion":null}"#)
            .bind(r#"{"summary":"not confirmed","outcomes":[],"confirmedFacts":[],"unconfirmedFacts":[],"skillCandidates":[],"nextQuestion":null}"#)
            .execute(pool).await.unwrap();
        sqlx::query("INSERT INTO skill_observations (id, activity_id, analysis_id, skill_id, specialized_skill_name, normalized_specialized_skill_name, evidence, source, created_at) VALUES ('observation-1', 'activity-1', 'analysis-1', 'technical.validation', 'SQL performance diagnosis', 'sql performance diagnosis', 'user confirmed evidence', 'analysis_confirmation', '2026-07-20T00:00:00Z')")
            .execute(pool).await.unwrap();
        state
    }

    #[tokio::test]
    async fn quest_payload_includes_only_confirmed_context_and_evidence() {
        let state = prepared_state().await;
        let payload = quest_payload(&state, "activity-1", "analysis-1")
            .await
            .unwrap();
        let text = serde_json::to_string(&payload).unwrap();
        assert!(text.contains("confirmed fact"));
        assert!(text.contains("confirmed outcome"));
        assert!(text.contains("SQL performance diagnosis"));
        assert!(!text.contains("RAW ACTIVITY MUST NOT LEAK"));
        assert!(!text.contains("UNCONFIRMED MUST NOT LEAK"));
        assert!(!text.contains("AI assumption"));
        assert_eq!(payload["focusThemes"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn quest_payload_rejects_unconfirmed_analysis_and_validates_edited_preview() {
        let state = prepared_state().await;
        let error = quest_payload(&state, "activity-1", "analysis-unconfirmed")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("確認済み"));
        let edited = r#"{"schemaVersion":"edited-by-user","note":"shorten this quest"}"#;
        assert_eq!(validate_submitted_payload(edited).unwrap(), edited);
        assert!(validate_submitted_payload("").is_err());
        assert!(validate_submitted_payload("not-json").is_err());
        assert!(validate_submitted_payload("[]").is_err());
        assert!(validate_submitted_payload(&"x".repeat(MAX_SUBMITTED_PAYLOAD_BYTES + 1)).is_err());
    }
}
