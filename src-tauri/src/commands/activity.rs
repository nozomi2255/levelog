use std::path::PathBuf;

use sqlx::{Row, sqlite::SqliteRow};
use tauri::State;
use tokio::sync::watch;
use uuid::Uuid;

use crate::{
    application::{
        CandidateDecision, CandidateDecisionValue, NewActivity, NewAnalysis, ServiceError,
    },
    dto::{
        ActivityAnalysisDto, ActivityDetailDto, ActivityDto, ActivityInboxItemDto,
        ActivityWorkflowDto, AnalysisJobDto, AnalysisPreview, ConfirmAnalysisInput,
        ConfirmAnalysisResult, CreateActivityInput, InterviewAnswerInput, InterviewChoiceDto,
        InterviewQuestionDto, NextQuestionOutput, QuickCaptureInput, SkillCandidateDto,
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
pub async fn quick_capture_activity(
    state: State<'_, AppState>,
    input: QuickCaptureInput,
) -> Result<ActivityDto, AppError> {
    if input.raw_text.trim().is_empty() {
        return Err(AppError::Validation("経験を一言入力してください".into()));
    }
    if input.raw_text.chars().count() > 20_000 {
        return Err(AppError::Validation(
            "経験の原文は20000文字以内で入力してください".into(),
        ));
    }
    if !matches!(input.capture_mode.as_str(), "quick" | "guided" | "deep") {
        return Err(AppError::Validation(
            "入力の深さはquick・guided・deepから選択してください".into(),
        ));
    }
    let id = state
        .growth
        .quick_capture_activity(&input.occurred_on, &input.raw_text, &input.capture_mode)
        .await?;
    activity_by_id(&state, &id).await
}

#[tauri::command]
pub async fn list_activity_inbox(
    state: State<'_, AppState>,
) -> Result<Vec<ActivityInboxItemDto>, AppError> {
    let activity_ids: Vec<String> = sqlx::query_scalar(
        "SELECT activity_id FROM activity_workflows WHERE state NOT IN ('confirmed', 'excluded') ORDER BY updated_at DESC",
    )
    .fetch_all(state.db.pool())
    .await?;
    let mut items = Vec::with_capacity(activity_ids.len());
    for activity_id in activity_ids {
        items.push(ActivityInboxItemDto {
            activity: activity_by_id(&state, &activity_id).await?,
            workflow: workflow_by_activity_id(&state, &activity_id).await?,
        });
    }
    Ok(items)
}

#[tauri::command]
pub async fn get_activity_workflow(
    state: State<'_, AppState>,
    activity_id: String,
) -> Result<ActivityWorkflowDto, AppError> {
    workflow_by_activity_id(&state, &activity_id).await
}

#[tauri::command]
pub async fn answer_activity_question(
    state: State<'_, AppState>,
    input: InterviewAnswerInput,
) -> Result<ActivityWorkflowDto, AppError> {
    answer_activity_question_inner(&state, input).await
}

async fn answer_activity_question_inner(
    state: &AppState,
    input: InterviewAnswerInput,
) -> Result<ActivityWorkflowDto, AppError> {
    let session = sqlx::query(
        "SELECT activity_id, status, current_question_json FROM interview_sessions WHERE id = ?",
    )
    .bind(&input.session_id)
    .fetch_optional(state.db.pool())
    .await?
    .ok_or_else(|| AppError::NotFound("interview session".into()))?;
    let activity_id: String = session.get("activity_id");
    let status: String = session.get("status");
    let question: NextQuestionOutput =
        serde_json::from_str(&session.get::<String, _>("current_question_json"))
            .map_err(|error| AppError::Internal(error.to_string()))?;
    if input.question_id != question.question_id {
        return Err(AppError::Validation("質問IDが一致しません".into()));
    }
    if !matches!(
        input.answer_state.as_str(),
        "answered" | "unknown" | "skipped" | "deferred"
    ) {
        return Err(AppError::Validation("回答状態が正しくありません".into()));
    }
    let submitted_answer = input
        .answer
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let answer = if input.answer_state == "answered" {
        let answer = submitted_answer
            .ok_or_else(|| AppError::Validation("回答を入力してください".into()))?;
        if question.answer_type == "single_choice"
            && !question.choices.iter().any(|choice| choice.value == answer)
        {
            return Err(AppError::Validation("選択肢から回答してください".into()));
        }
        if question.answer_type == "number" && answer.parse::<f64>().is_err() {
            return Err(AppError::Validation("数値で回答してください".into()));
        }
        Some(answer)
    } else {
        None
    };
    let answer_json = answer.map(|value| serde_json::json!({ "answer": value }).to_string());
    let latest_answer = sqlx::query(
        "SELECT answer_json, answer_state FROM interview_answers WHERE session_id = ? AND question_id = ? ORDER BY created_at DESC, rowid DESC LIMIT 1",
    )
    .bind(&input.session_id)
    .bind(&input.question_id)
    .fetch_optional(state.db.pool())
    .await?;
    if let Some(latest_answer) = latest_answer {
        let latest_state: String = latest_answer.get("answer_state");
        let latest_json: Option<String> = latest_answer.get("answer_json");
        let is_same_submission =
            latest_state == input.answer_state && latest_json.as_deref() == answer_json.as_deref();
        if matches!(latest_state.as_str(), "answered" | "unknown" | "skipped") {
            if is_same_submission {
                return workflow_by_activity_id(state, &activity_id).await;
            }
            return Err(AppError::InvalidState(
                "この質問にはすでに別の回答を確定しています".into(),
            ));
        }
        if latest_state == "deferred" && is_same_submission {
            return workflow_by_activity_id(state, &activity_id).await;
        }
    }
    if !matches!(status.as_str(), "pending" | "deferred") {
        return Err(AppError::InvalidState(
            "この質問にはすでに回答しています".into(),
        ));
    }
    let now = super::app::now();
    let mut tx = state.db.pool().begin().await?;
    sqlx::query("INSERT INTO interview_answers (id, session_id, question_id, answer_json, answer_state, created_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(Uuid::new_v4().to_string())
        .bind(&input.session_id)
        .bind(&input.question_id)
        .bind(&answer_json)
        .bind(&input.answer_state)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE interview_sessions SET status = ?, updated_at = ? WHERE id = ?")
        .bind(&input.answer_state)
        .bind(&now)
        .bind(&input.session_id)
        .execute(&mut *tx)
        .await?;
    let workflow_state = if input.answer_state == "deferred" {
        "needs_input"
    } else {
        "assessable"
    };
    sqlx::query("UPDATE activity_workflows SET state = ?, version = version + 1, updated_at = ? WHERE activity_id = ?")
        .bind(workflow_state)
        .bind(&now)
        .bind(&activity_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    workflow_by_activity_id(state, &activity_id).await
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
    let profile_json: Option<String> = sqlx::query_scalar(
        "SELECT profile_json FROM user_profile_revisions ORDER BY revision DESC LIMIT 1",
    )
    .fetch_optional(state.db.pool())
    .await?;
    let explicit_profile = if let Some(profile_json) = profile_json {
        serde_json::from_str::<serde_json::Value>(&profile_json)
            .map_err(|error| AppError::Internal(error.to_string()))?
    } else {
        load_profile(&state)
            .await?
            .map(|profile| {
                serde_json::json!({
                    "role": profile.role,
                    "focusSkillIds": profile.focus_skill_ids,
                    "weeklyMinutes": profile.weekly_minutes,
                    "questConstraints": profile.excluded_quest_patterns,
                })
            })
            .unwrap_or(serde_json::Value::Null)
    };
    let raw_text: Option<String> =
        sqlx::query_scalar("SELECT raw_text FROM activity_captures WHERE activity_id = ?")
            .bind(&activity_id)
            .fetch_optional(state.db.pool())
            .await?;
    let interview_rows = sqlx::query("SELECT s.current_question_json, a.answer_json, a.answer_state FROM interview_answers a JOIN interview_sessions s ON s.id = a.session_id WHERE s.activity_id = ? AND a.answer_state != 'deferred' ORDER BY a.created_at")
        .bind(&activity_id)
        .fetch_all(state.db.pool())
        .await?;
    let interview_answers = interview_rows.iter().map(|row| serde_json::json!({
        "question": serde_json::from_str::<serde_json::Value>(&row.get::<String, _>("current_question_json")).unwrap_or(serde_json::Value::Null),
        "answer": row.get::<Option<String>, _>("answer_json").and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok()),
        "answerState": row.get::<String, _>("answer_state"),
    })).collect::<Vec<_>>();
    let payload = serde_json::json!({
        "activity": {
            "occurredOn": activity.occurred_on,
            "rawText": raw_text.unwrap_or_else(|| activity.action_text.clone()),
            "whatIDid": activity.action_text,
            "whatWasDifficult": activity.challenge_text,
            "whatChanged": activity.outcome_text,
        },
        "explicitProfile": explicit_profile,
        "interviewAnswers": interview_answers,
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
            prompt_version: "activity-analysis.v2".into(),
            schema_version: "activity-analysis.v2".into(),
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
                save_analysis_error(&task_state, &task_id, "cancelled", "ユーザーが解析をキャンセルしました", None).await;
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
                None,
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
                None,
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
                        let retryable = result.as_ref().is_err_and(CodexError::is_schema_retryable);
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
            Ok(result) => {
                let raw = result.raw_json;
                let output = result.parsed;
                let candidates = output
                    .skill_candidates
                    .into_iter()
                    .map(|candidate| {
                        (
                            candidate.skill_id,
                            candidate.specialized_skill_name,
                            candidate.confidence,
                            candidate.reason,
                            candidate.evidence,
                        )
                    })
                    .collect();
                let next_question_json = output
                    .next_question
                    .as_ref()
                    .and_then(|question| serde_json::to_string(question).ok());
                if let Err(error) = task_state
                    .growth
                    .save_analysis_result(
                        &task_id,
                        &raw,
                        candidates,
                        next_question_json.as_deref(),
                        "activity-analysis.v2",
                        "activity-analysis.v2",
                    )
                    .await
                    && !matches!(error, ServiceError::AnalysisNotRunning)
                {
                    save_analysis_error(&task_state, &task_id, "failed", &error.to_string(), None)
                        .await;
                }
            }
            Err(error) => {
                let status = if matches!(error, CodexError::Cancelled) {
                    "cancelled"
                } else {
                    "failed"
                };
                save_analysis_error(
                    &task_state,
                    &task_id,
                    status,
                    &error.to_string(),
                    error.raw_output(),
                )
                .await;
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
                edited_skill_id: decision.edited_skill_id,
                edited_specialized_skill_name: decision.edited_specialized_skill_name,
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
    let parsed = raw.and_then(|json| crate::dto::parse_activity_analysis_output_compat(&json).ok());
    let candidates = sqlx::query("SELECT id, skill_id, specialized_skill_name, confidence, COALESCE(edited_reason, reason) reason, COALESCE(edited_evidence, evidence) evidence, decision FROM skill_candidates WHERE analysis_id = ? ORDER BY confidence DESC")
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
            specialized_skill_name: candidate.get("specialized_skill_name"),
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
        confirmed_facts: parsed
            .as_ref()
            .map(|value| value.confirmed_facts.clone())
            .unwrap_or_default(),
        unconfirmed_facts: parsed
            .as_ref()
            .map(|value| value.unconfirmed_facts.clone())
            .unwrap_or_default(),
        skill_candidates: candidates,
        missing_information_question: parsed
            .as_ref()
            .and_then(|value| value.missing_information_question.clone()),
        next_question: current_question_for_analysis(state, id).await?,
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

async fn save_analysis_error(
    state: &AppState,
    id: &str,
    status: &str,
    message: &str,
    raw_output: Option<&str>,
) {
    let _ = sqlx::query(
        "UPDATE ai_analyses SET status = ?, error_message = ?, raw_result_json = COALESCE(?, raw_result_json), completed_at = ? WHERE id = ? AND status IN ('pending', 'running')",
    )
    .bind(status)
    .bind(message)
    .bind(raw_output)
    .bind(super::app::now())
    .bind(id)
    .execute(state.db.pool())
    .await;
    let _ = sqlx::query("UPDATE activity_workflows SET state = 'assessable', version = version + 1, updated_at = ? WHERE activity_id = (SELECT activity_id FROM ai_analyses WHERE id = ?)")
        .bind(super::app::now())
        .bind(id)
        .execute(state.db.pool())
        .await;
}

async fn workflow_by_activity_id(
    state: &AppState,
    activity_id: &str,
) -> Result<ActivityWorkflowDto, AppError> {
    let row = sqlx::query(
        "SELECT state, version, updated_at FROM activity_workflows WHERE activity_id = ?",
    )
    .bind(activity_id)
    .fetch_optional(state.db.pool())
    .await?
    .ok_or_else(|| AppError::NotFound("activity workflow".into()))?;
    Ok(ActivityWorkflowDto {
        activity_id: activity_id.into(),
        state: row.get("state"),
        version: row.get("version"),
        current_question: current_question_for_activity(state, activity_id).await?,
        updated_at: row.get("updated_at"),
    })
}

async fn current_question_for_activity(
    state: &AppState,
    activity_id: &str,
) -> Result<Option<InterviewQuestionDto>, AppError> {
    let row = sqlx::query("SELECT id, status, current_question_json FROM interview_sessions WHERE activity_id = ? AND status IN ('pending', 'deferred') ORDER BY created_at DESC LIMIT 1")
        .bind(activity_id)
        .fetch_optional(state.db.pool())
        .await?;
    row.map(question_from_session_row).transpose()
}

async fn current_question_for_analysis(
    state: &AppState,
    analysis_id: &str,
) -> Result<Option<InterviewQuestionDto>, AppError> {
    let row = sqlx::query("SELECT id, status, current_question_json FROM interview_sessions WHERE analysis_id = ? ORDER BY created_at DESC LIMIT 1")
        .bind(analysis_id)
        .fetch_optional(state.db.pool())
        .await?;
    row.map(question_from_session_row).transpose()
}

fn question_from_session_row(row: SqliteRow) -> Result<InterviewQuestionDto, AppError> {
    let output: NextQuestionOutput =
        serde_json::from_str(&row.get::<String, _>("current_question_json"))
            .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(InterviewQuestionDto {
        session_id: row.get("id"),
        question_id: output.question_id,
        target: output.target,
        text: output.text,
        answer_type: output.answer_type,
        choices: output
            .choices
            .into_iter()
            .map(|choice| InterviewChoiceDto {
                value: choice.value,
                label: choice.label,
            })
            .collect(),
        why_it_matters: output.why_it_matters,
        status: row.get("status"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn deferred_question_can_be_answered_later_and_terminal_replays_are_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::initialize(directory.path().to_path_buf())
            .await
            .unwrap();
        let activity_id = state
            .growth
            .quick_capture_activity("2026-07-20", "SQLを速くした", "guided")
            .await
            .unwrap();
        let analysis_id = state
            .growth
            .create_analysis(NewAnalysis {
                activity_id: activity_id.clone(),
                submitted_payload: "{}".into(),
                provider: "test".into(),
                model: None,
                codex_version: None,
                prompt_version: "v2".into(),
                schema_version: "v2".into(),
            })
            .await
            .unwrap();
        let question = r#"{"questionId":"measurement","target":"measurement","text":"どれくらい改善しましたか？","answerType":"text","choices":[],"whyItMatters":"成果を確認するため"}"#;
        sqlx::query(
            "UPDATE ai_analyses SET status = 'running' WHERE id = ? AND status = 'pending'",
        )
        .bind(&analysis_id)
        .execute(state.db.pool())
        .await
        .unwrap();
        state
            .growth
            .save_analysis_result(
                &analysis_id,
                "{}",
                vec![(
                    "thinking.problem_decomposition".into(),
                    Some("SQL性能調査".into()),
                    0.8,
                    "reason".into(),
                    "evidence".into(),
                )],
                Some(question),
                "v2",
                "v2",
            )
            .await
            .unwrap();
        let session_id: String =
            sqlx::query_scalar("SELECT id FROM interview_sessions WHERE analysis_id = ?")
                .bind(&analysis_id)
                .fetch_one(state.db.pool())
                .await
                .unwrap();

        let deferred = InterviewAnswerInput {
            session_id: session_id.clone(),
            question_id: "measurement".into(),
            answer_state: "deferred".into(),
            answer: None,
        };
        let first_defer = answer_activity_question_inner(&state, deferred.clone())
            .await
            .unwrap();
        let replayed_defer = answer_activity_question_inner(&state, deferred)
            .await
            .unwrap();
        assert_eq!(first_defer.state, "needs_input");
        assert_eq!(first_defer.version, replayed_defer.version);

        let answered = InterviewAnswerInput {
            session_id: session_id.clone(),
            question_id: "measurement".into(),
            answer_state: "answered".into(),
            answer: Some("30%".into()),
        };
        let first_answer = answer_activity_question_inner(&state, answered.clone())
            .await
            .unwrap();
        let replayed_answer = answer_activity_question_inner(&state, answered)
            .await
            .unwrap();
        assert_eq!(first_answer.state, "assessable");
        assert!(first_answer.current_question.is_none());
        assert_eq!(first_answer.version, replayed_answer.version);

        let different_answer = answer_activity_question_inner(
            &state,
            InterviewAnswerInput {
                session_id: session_id.clone(),
                question_id: "measurement".into(),
                answer_state: "answered".into(),
                answer: Some("40%".into()),
            },
        )
        .await;
        assert!(matches!(different_answer, Err(AppError::InvalidState(_))));
        let answer_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM interview_answers WHERE session_id = ?")
                .bind(&session_id)
                .fetch_one(state.db.pool())
                .await
                .unwrap();
        let session_status: String =
            sqlx::query_scalar("SELECT status FROM interview_sessions WHERE id = ?")
                .bind(&session_id)
                .fetch_one(state.db.pool())
                .await
                .unwrap();
        assert_eq!(answer_count, 2);
        assert_eq!(session_status, "answered");
    }

    #[tokio::test]
    async fn invalid_ai_output_is_retained_for_auditing() {
        let directory = tempfile::tempdir().unwrap();
        let state = AppState::initialize(directory.path().to_path_buf())
            .await
            .unwrap();
        let activity_id = state
            .growth
            .quick_capture_activity("2026-07-20", "SQLを確認した", "quick")
            .await
            .unwrap();
        let analysis_id = state
            .growth
            .create_analysis(NewAnalysis {
                activity_id,
                submitted_payload: "{}".into(),
                provider: "test".into(),
                model: None,
                codex_version: None,
                prompt_version: "v2".into(),
                schema_version: "v2".into(),
            })
            .await
            .unwrap();
        sqlx::query("UPDATE ai_analyses SET status = 'running' WHERE id = ?")
            .bind(&analysis_id)
            .execute(state.db.pool())
            .await
            .unwrap();
        save_analysis_error(
            &state,
            &analysis_id,
            "failed",
            "schema violation",
            Some("not-json-from-codex"),
        )
        .await;
        let stored: (String, String) =
            sqlx::query_as("SELECT status, raw_result_json FROM ai_analyses WHERE id = ?")
                .bind(&analysis_id)
                .fetch_one(state.db.pool())
                .await
                .unwrap();
        assert_eq!(stored.0, "failed");
        assert_eq!(stored.1, "not-json-from-codex");
    }
}
