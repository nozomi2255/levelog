use std::path::PathBuf;

use chrono::Local;
use sqlx::Row;
use tauri::State;
use tokio::sync::watch;

use crate::{
    application::NewQuest,
    domain::{QuestStatus, ReflectionResult},
    dto::{
        GenerateQuestInput, QuestDto, QuestReflectionDto, QuestReflectionInput,
        QuestTransitionInput,
    },
    error::AppError,
    infrastructure::codex::{CodexClient, CodexError, TIMEOUT, TokioProcessRunner},
    state::AppState,
};

use super::app::{load_profile, quest_from_row};

#[tauri::command]
pub async fn generate_quest(
    state: State<'_, AppState>,
    input: GenerateQuestInput,
) -> Result<QuestDto, AppError> {
    let profile = load_profile(&state)
        .await?
        .ok_or_else(|| AppError::InvalidState("初期設定を完了してください".into()))?;
    let analysis = sqlx::query(
        "SELECT status, raw_result_json FROM ai_analyses WHERE id = ? AND activity_id = ?",
    )
    .bind(&input.analysis_id)
    .bind(&input.activity_id)
    .fetch_optional(state.db.pool())
    .await?
    .ok_or_else(|| AppError::NotFound("analysis".into()))?;
    let status: String = analysis.get("status");
    if status != "confirmed" {
        return Err(AppError::InvalidState(
            "確認済みの分析からのみクエストを作成できます".into(),
        ));
    }
    let raw_result: Option<String> = analysis.get("raw_result_json");
    let parsed = raw_result
        .as_deref()
        .and_then(|json| serde_json::from_str::<crate::dto::ActivityAnalysisOutput>(json).ok());
    let confirmed_evidence = sqlx::query(
        "SELECT skill_id, evidence FROM skill_observations WHERE analysis_id = ? ORDER BY created_at",
    )
    .bind(&input.analysis_id)
    .fetch_all(state.db.pool())
    .await?
    .iter()
    .map(|row| {
        serde_json::json!({
            "skillId": row.get::<String, _>("skill_id"),
            "evidence": row.get::<String, _>("evidence"),
        })
    })
    .collect::<Vec<_>>();
    let payload = serde_json::json!({
        "role": profile.role,
        "focusSkillIds": profile.focus_skill_ids,
        "weeklyMinutes": profile.weekly_minutes,
        "excludedQuestPatterns": profile.excluded_quest_patterns,
        "confirmedAnalysis": {
            "summary": parsed.as_ref().map(|value| value.summary.as_str()),
            "outcomes": parsed.as_ref().map(|value| value.outcomes.as_slice()).unwrap_or_default(),
            "evidence": confirmed_evidence,
        },
    });
    let client = CodexClient::new(PathBuf::from(profile.codex_path), TokioProcessRunner)?;
    let _permit = state
        .codex_semaphore
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| AppError::Internal("Codex実行キューを開始できませんでした".into()))?;
    let (_cancel_sender, cancel_receiver) = watch::channel(false);
    let serialized_payload =
        serde_json::to_string(&payload).map_err(|error| AppError::Internal(error.to_string()))?;
    let execution = async {
        let mut attempt = 0;
        loop {
            let result = client
                .propose_quest(serialized_payload.clone(), cancel_receiver.clone())
                .await;
            let retryable = matches!(
                result,
                Err(CodexError::InvalidJson(_) | CodexError::SchemaViolation(_, _))
            );
            if retryable && attempt == 0 {
                attempt += 1;
                continue;
            }
            break result;
        }
    };
    let proposal = tokio::time::timeout(TIMEOUT, execution)
        .await
        .map_err(|_| CodexError::TimedOut)??;
    let id = state
        .growth
        .create_quest(NewQuest {
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
        })
        .await?;
    quest_by_id(&state, &id).await
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
