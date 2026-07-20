use std::path::PathBuf;

use sqlx::{Row, sqlite::SqliteRow};
use tauri::State;
use tokio::sync::watch;

use crate::{
    application::{CandidateDecision, CandidateDecisionValue, NewActivity, NewAnalysis},
    dto::{
        ActivityAnalysisDto, ActivityDetailDto, ActivityDto, AnalysisJobDto, AnalysisPreview,
        ConfirmAnalysisInput, ConfirmAnalysisResult, CreateActivityInput, SkillCandidateDto,
        StartAnalysisInput,
    },
    error::AppError,
    infrastructure::codex::{CodexClient, CodexError, TokioProcessRunner},
    state::AppState,
};

use super::app::{activity_from_row, load_profile};

#[tauri::command]
pub async fn create_activity(
    state: State<'_, AppState>,
    input: CreateActivityInput,
) -> Result<ActivityDto, AppError> {
    if input.action_text.trim().is_empty()
        && input.challenge_text.trim().is_empty()
        && input.outcome_text.trim().is_empty()
    {
        return Err(AppError::Validation(
            "活動、難しかったこと、変化のいずれかを入力してください".into(),
        ));
    }
    let id = state
        .growth
        .create_activity(NewActivity {
            occurred_on: input.occurred_on,
            action_text: input.action_text.trim().into(),
            challenge_text: input.challenge_text.trim().into(),
            outcome_text: input.outcome_text.trim().into(),
        })
        .await?;
    activity_by_id(&state, &id).await
}

#[tauri::command]
pub async fn list_activities(state: State<'_, AppState>) -> Result<Vec<ActivityDto>, AppError> {
    let rows = sqlx::query("SELECT a.*, (SELECT status FROM ai_analyses x WHERE x.activity_id = a.id ORDER BY x.created_at DESC LIMIT 1) analysis_status FROM activities a ORDER BY occurred_on DESC, created_at DESC")
        .fetch_all(state.db.pool())
        .await?;
    Ok(rows.iter().map(activity_from_row).collect())
}

#[tauri::command]
pub async fn get_activity(
    state: State<'_, AppState>,
    activity_id: String,
) -> Result<ActivityDetailDto, AppError> {
    let activity = activity_by_id(&state, &activity_id).await?;
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM ai_analyses WHERE activity_id = ? ORDER BY created_at DESC",
    )
    .bind(&activity_id)
    .fetch_all(state.db.pool())
    .await?;
    let mut analyses = Vec::with_capacity(ids.len());
    for id in ids {
        analyses.push(analysis_by_id(&state, &id).await?);
    }
    Ok(ActivityDetailDto { activity, analyses })
}

#[tauri::command]
pub async fn get_analysis_preview(
    state: State<'_, AppState>,
    activity_id: String,
) -> Result<AnalysisPreview, AppError> {
    let activity = activity_by_id(&state, &activity_id).await?;
    let profile = load_profile(&state).await?;
    let payload = serde_json::json!({
        "activity": {
            "occurredOn": activity.occurred_on,
            "whatIDid": activity.action_text,
            "whatWasDifficult": activity.challenge_text,
            "whatChanged": activity.outcome_text,
        },
        "profile": profile.map(|profile| serde_json::json!({
            "role": profile.role,
            "focusSkillIds": profile.focus_skill_ids,
        })),
    });
    Ok(AnalysisPreview {
        activity_id,
        submitted_payload: serde_json::to_string_pretty(&payload)
            .map_err(|error| AppError::Internal(error.to_string()))?,
        cloud_inference_notice:
            "この内容はCodexの推論先へ送信されます。顧客名・秘密情報を確認し、必要なら伏字にしてください。"
                .into(),
    })
}

#[tauri::command]
pub async fn start_activity_analysis(
    state: State<'_, AppState>,
    input: StartAnalysisInput,
) -> Result<AnalysisJobDto, AppError> {
    activity_by_id(&state, &input.activity_id).await?;
    let profile = load_profile(&state)
        .await?
        .ok_or_else(|| AppError::InvalidState("初期設定を完了してください".into()))?;
    serde_json::from_str::<serde_json::Value>(&input.submitted_payload)
        .map_err(|error| AppError::Validation(format!("送信JSONが正しくありません: {error}")))?;

    let analysis_id = state
        .growth
        .create_analysis(NewAnalysis {
            activity_id: input.activity_id,
            submitted_payload: input.submitted_payload.clone(),
            provider: "codex-cli".into(),
            model: None,
            codex_version: None,
            prompt_version: "activity-analysis.v1".into(),
            schema_version: "activity-analysis.v1".into(),
        })
        .await?;
    let (cancel_sender, cancel_receiver) = watch::channel(false);
    state
        .analysis_cancellations
        .lock()
        .await
        .insert(analysis_id.clone(), cancel_sender);
    let task_state = state.inner().clone();
    let task_id = analysis_id.clone();
    let payload = input.submitted_payload;
    tauri::async_runtime::spawn(async move {
        let mut queued_cancel = cancel_receiver.clone();
        let permit = tokio::select! {
            permit = task_state.codex_semaphore.clone().acquire_owned() => permit,
            _ = queued_cancel.changed() => {
                save_analysis_error(&task_state, &task_id, "cancelled", "ユーザーが解析をキャンセルしました").await;
                task_state.analysis_cancellations.lock().await.remove(&task_id);
                return;
            }
        };
        let Ok(_permit) = permit else {
            save_analysis_error(
                &task_state,
                &task_id,
                "failed",
                "Codex実行キューを開始できませんでした",
            )
            .await;
            task_state
                .analysis_cancellations
                .lock()
                .await
                .remove(&task_id);
            return;
        };
        if *cancel_receiver.borrow() {
            save_analysis_error(
                &task_state,
                &task_id,
                "cancelled",
                "ユーザーが解析をキャンセルしました",
            )
            .await;
            task_state
                .analysis_cancellations
                .lock()
                .await
                .remove(&task_id);
            return;
        }
        let running = sqlx::query(
            "UPDATE ai_analyses SET status = 'running' WHERE id = ? AND status = 'pending'",
        )
        .bind(&task_id)
        .execute(task_state.db.pool())
        .await;
        if !matches!(running, Ok(result) if result.rows_affected() == 1) {
            task_state
                .analysis_cancellations
                .lock()
                .await
                .remove(&task_id);
            return;
        }
        let client = CodexClient::new(PathBuf::from(profile.codex_path), TokioProcessRunner);
        let result = match client {
            Ok(client) => {
                let execution = async {
                    let mut attempt = 0;
                    loop {
                        let result = client
                            .analyze_activity(payload.clone(), cancel_receiver.clone())
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
                let mut execution_cancel = cancel_receiver.clone();
                tokio::select! {
                    result = tokio::time::timeout(crate::infrastructure::codex::TIMEOUT, execution) => result.unwrap_or(Err(CodexError::TimedOut)),
                    _ = execution_cancel.changed() => Err(CodexError::Cancelled),
                }
            }
            Err(error) => Err(error),
        };
        match result {
            Ok(output) => {
                let raw = serde_json::to_string(&output).unwrap_or_else(|_| "{}".into());
                let candidates = output
                    .skill_candidates
                    .into_iter()
                    .map(|candidate| {
                        (
                            candidate.skill_id,
                            candidate.confidence,
                            candidate.reason,
                            candidate.evidence,
                        )
                    })
                    .collect();
                if let Err(error) = task_state
                    .growth
                    .save_analysis_result(&task_id, &raw, candidates)
                    .await
                {
                    save_analysis_error(&task_state, &task_id, "failed", &error.to_string()).await;
                }
            }
            Err(error) => {
                let status = if matches!(error, CodexError::Cancelled) {
                    "cancelled"
                } else {
                    "failed"
                };
                save_analysis_error(&task_state, &task_id, status, &error.to_string()).await;
            }
        }
        task_state
            .analysis_cancellations
            .lock()
            .await
            .remove(&task_id);
    });
    analysis_job_by_id(&state, &analysis_id).await
}

#[tauri::command]
pub async fn get_activity_analysis(
    state: State<'_, AppState>,
    analysis_id: String,
) -> Result<ActivityAnalysisDto, AppError> {
    analysis_by_id(&state, &analysis_id).await
}

#[tauri::command]
pub async fn cancel_activity_analysis(
    state: State<'_, AppState>,
    analysis_id: String,
) -> Result<AnalysisJobDto, AppError> {
    if let Some(sender) = state
        .analysis_cancellations
        .lock()
        .await
        .remove(&analysis_id)
    {
        let _ = sender.send(true);
    }
    sqlx::query("UPDATE ai_analyses SET status = 'cancelled', error_message = 'ユーザーが解析をキャンセルしました', completed_at = ? WHERE id = ? AND status IN ('pending', 'running')")
        .bind(super::app::now())
        .bind(&analysis_id)
        .execute(state.db.pool())
        .await?;
    analysis_job_by_id(&state, &analysis_id).await
}

#[tauri::command]
pub async fn confirm_activity_analysis(
    state: State<'_, AppState>,
    input: ConfirmAnalysisInput,
) -> Result<ConfirmAnalysisResult, AppError> {
    let previous_status: String = sqlx::query_scalar("SELECT status FROM ai_analyses WHERE id = ?")
        .bind(&input.analysis_id)
        .fetch_optional(state.db.pool())
        .await?
        .ok_or_else(|| AppError::NotFound("analysis".into()))?;
    let decisions = input
        .candidate_decisions
        .into_iter()
        .map(|decision| {
            let value = match decision.decision.as_str() {
                "accepted" => Ok(CandidateDecisionValue::Accepted),
                "rejected" => Ok(CandidateDecisionValue::Rejected),
                "edited" => Ok(CandidateDecisionValue::Edited),
                _ => Err(AppError::Validation(format!(
                    "候補の判断が正しくありません: {}",
                    decision.decision
                ))),
            }?;
            Ok(CandidateDecision {
                candidate_id: decision.candidate_id,
                decision: value,
                edited_reason: decision.edited_reason,
                edited_evidence: decision.edited_evidence,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    state
        .growth
        .confirm_analysis(&input.analysis_id, decisions)
        .await?;
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM skill_observations WHERE analysis_id = ?")
            .bind(&input.analysis_id)
            .fetch_one(state.db.pool())
            .await?;
    Ok(ConfirmAnalysisResult {
        analysis_id: input.analysis_id,
        confirmed_observation_count: count,
        xp_awarded: if previous_status == "confirmed" {
            0
        } else {
            20
        },
    })
}

async fn activity_by_id(state: &AppState, id: &str) -> Result<ActivityDto, AppError> {
    let row = sqlx::query("SELECT a.*, (SELECT status FROM ai_analyses x WHERE x.activity_id = a.id ORDER BY x.created_at DESC LIMIT 1) analysis_status FROM activities a WHERE a.id = ?")
        .bind(id)
        .fetch_optional(state.db.pool())
        .await?
        .ok_or_else(|| AppError::NotFound("activity".into()))?;
    Ok(activity_from_row(&row))
}

async fn analysis_job_by_id(state: &AppState, id: &str) -> Result<AnalysisJobDto, AppError> {
    let row = sqlx::query("SELECT id, activity_id, status, error_message, created_at, completed_at FROM ai_analyses WHERE id = ?")
        .bind(id)
        .fetch_optional(state.db.pool())
        .await?
        .ok_or_else(|| AppError::NotFound("analysis".into()))?;
    Ok(job_from_row(&row))
}

async fn analysis_by_id(state: &AppState, id: &str) -> Result<ActivityAnalysisDto, AppError> {
    let row = sqlx::query("SELECT * FROM ai_analyses WHERE id = ?")
        .bind(id)
        .fetch_optional(state.db.pool())
        .await?
        .ok_or_else(|| AppError::NotFound("analysis".into()))?;
    let raw: Option<String> = row.get("raw_result_json");
    let parsed =
        raw.and_then(|json| serde_json::from_str::<crate::dto::ActivityAnalysisOutput>(&json).ok());
    let candidates = sqlx::query("SELECT id, skill_id, confidence, COALESCE(edited_reason, reason) reason, COALESCE(edited_evidence, evidence) evidence, decision FROM skill_candidates WHERE analysis_id = ? ORDER BY confidence DESC")
        .bind(id)
        .fetch_all(state.db.pool())
        .await?
        .iter()
        .map(|candidate| SkillCandidateDto {
            id: candidate.get("id"),
            skill_id: candidate.get("skill_id"),
            confidence: candidate.get("confidence"),
            reason: candidate.get("reason"),
            evidence: candidate.get("evidence"),
            decision: candidate.get("decision"),
        })
        .collect();
    Ok(ActivityAnalysisDto {
        id: row.get("id"),
        activity_id: row.get("activity_id"),
        status: row.get("status"),
        summary: parsed.as_ref().map(|value| value.summary.clone()),
        outcomes: parsed
            .as_ref()
            .map(|value| value.outcomes.clone())
            .unwrap_or_default(),
        skill_candidates: candidates,
        missing_information_question: parsed.and_then(|value| value.missing_information_question),
        error_message: row.get("error_message"),
    })
}

fn job_from_row(row: &SqliteRow) -> AnalysisJobDto {
    AnalysisJobDto {
        id: row.get("id"),
        activity_id: row.get("activity_id"),
        status: row.get("status"),
        error_message: row.get("error_message"),
        created_at: row.get("created_at"),
        completed_at: row.get("completed_at"),
    }
}

async fn save_analysis_error(state: &AppState, id: &str, status: &str, message: &str) {
    let _ = sqlx::query(
        "UPDATE ai_analyses SET status = ?, error_message = ?, completed_at = ? WHERE id = ?",
    )
    .bind(status)
    .bind(message)
    .bind(super::app::now())
    .bind(id)
    .execute(state.db.pool())
    .await;
}
