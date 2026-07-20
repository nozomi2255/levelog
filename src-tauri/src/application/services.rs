use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{Row, Sqlite, Transaction};
use std::collections::HashSet;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    domain::{QuestStatus, ReflectionResult, XpReason, level_for_total_xp},
    infrastructure::database::Database,
};

const ACTIVITY_XP: i64 = 10;
const ANALYSIS_XP: i64 = 20;
const REFLECTION_XP: i64 = 40;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("not found: {0}")]
    NotFound(&'static str),
    #[error("analysis must have succeeded before it can be confirmed")]
    AnalysisNotConfirmable,
    #[error("analysis is no longer running")]
    AnalysisNotRunning,
    #[error("an analysis is already pending or running for this activity")]
    AnalysisAlreadyRunning,
    #[error("the activity has an unanswered interview question")]
    InterviewQuestionPending,
    #[error("candidate {0} does not belong to analysis")]
    InvalidCandidate(String),
    #[error("invalid edited candidate: {0}")]
    InvalidCandidateEdit(String),
    #[error("every analysis candidate must have exactly one decision")]
    IncompleteCandidateDecisions,
    #[error("skill {0} is not in the fixed catalog")]
    UnknownSkill(String),
    #[error("invalid quest transition from {from} to {to}")]
    InvalidQuestTransition { from: String, to: String },
    #[error("quest must be completed before reflection")]
    QuestNotReflectable,
    #[error("quest generation run is no longer active")]
    QuestGenerationNotRunning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewActivity {
    pub occurred_on: String,
    pub action_text: String,
    pub challenge_text: String,
    pub outcome_text: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewAnalysis {
    pub activity_id: String,
    pub submitted_payload: String,
    pub provider: String,
    pub model: Option<String>,
    pub codex_version: Option<String>,
    pub prompt_version: String,
    pub schema_version: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateDecision {
    pub candidate_id: String,
    pub decision: CandidateDecisionValue,
    pub edited_reason: Option<String>,
    pub edited_evidence: Option<String>,
    pub edited_skill_id: Option<String>,
    pub edited_specialized_skill_name: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateDecisionValue {
    Accepted,
    Rejected,
    Edited,
}

impl CandidateDecisionValue {
    fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Edited => "edited",
        }
    }

    fn creates_observation(self) -> bool {
        matches!(self, Self::Accepted | Self::Edited)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewQuest {
    pub template_id: String,
    pub title: String,
    pub description: String,
    pub target_skill_id: Option<String>,
    pub difficulty: i64,
    pub estimated_minutes: i64,
    pub success_criteria_json: String,
    pub evidence_prompt: String,
    pub scheduled_on: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XpSummary {
    pub total_xp: i64,
    pub level: i64,
}

struct XpEvent<'a> {
    amount: i64,
    reason: XpReason,
    key: &'a str,
    activity_id: Option<&'a str>,
    analysis_id: Option<&'a str>,
    quest_id: Option<&'a str>,
    description: &'a str,
}

#[derive(Clone)]
pub struct GrowthService {
    db: Database,
}
impl GrowthService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
    pub fn db(&self) -> &Database {
        &self.db
    }

    pub async fn create_activity(&self, input: NewActivity) -> Result<String, ServiceError> {
        let id = Uuid::new_v4().to_string();
        let now = utc_now();
        let mut tx = self.db.pool().begin().await?;
        sqlx::query("INSERT INTO activities (id, occurred_on, action_text, challenge_text, outcome_text, created_at) VALUES (?, ?, ?, ?, ?, ?)")
            .bind(&id).bind(&input.occurred_on).bind(&input.action_text).bind(&input.challenge_text).bind(&input.outcome_text).bind(&now).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO activity_workflows (activity_id, state, version, updated_at) VALUES (?, 'assessable', 1, ?)")
            .bind(&id)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        let reason_key = format!("activity:{id}");
        self.award_xp(
            &mut tx,
            XpEvent {
                amount: ACTIVITY_XP,
                reason: XpReason::ActivitySaved,
                key: &reason_key,
                activity_id: Some(&id),
                analysis_id: None,
                quest_id: None,
                description: "活動を記録",
            },
        )
        .await?;
        tx.commit().await?;
        Ok(id)
    }

    /// Saves the user's unstructured capture, its compatibility activity row, workflow state,
    /// and deterministic activity XP as one unit.
    pub async fn quick_capture_activity(
        &self,
        occurred_on: &str,
        raw_text: &str,
        capture_mode: &str,
    ) -> Result<String, ServiceError> {
        let id = Uuid::new_v4().to_string();
        let capture_id = Uuid::new_v4().to_string();
        let now = utc_now();
        let mut tx = self.db.pool().begin().await?;
        sqlx::query("INSERT INTO activities (id, occurred_on, action_text, challenge_text, outcome_text, created_at) VALUES (?, ?, ?, '', '', ?)")
            .bind(&id)
            .bind(occurred_on)
            .bind(raw_text)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO activity_captures (id, activity_id, raw_text, capture_mode, created_at) VALUES (?, ?, ?, ?, ?)")
            .bind(capture_id)
            .bind(&id)
            .bind(raw_text)
            .bind(capture_mode)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO activity_workflows (activity_id, state, version, updated_at) VALUES (?, 'captured', 1, ?)")
            .bind(&id)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        let reason_key = format!("activity:{id}");
        self.award_xp(
            &mut tx,
            XpEvent {
                amount: ACTIVITY_XP,
                reason: XpReason::ActivitySaved,
                key: &reason_key,
                activity_id: Some(&id),
                analysis_id: None,
                quest_id: None,
                description: "活動を記録",
            },
        )
        .await?;
        tx.commit().await?;
        Ok(id)
    }

    pub async fn create_analysis(&self, input: NewAnalysis) -> Result<String, ServiceError> {
        let id = Uuid::new_v4().to_string();
        let activity_id = input.activity_id.clone();
        let mut tx = self.db.pool().begin().await?;
        let open_question_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM interview_sessions WHERE activity_id = ? AND status IN ('pending', 'deferred')",
        )
        .bind(&activity_id)
        .fetch_one(&mut *tx)
        .await?;
        if open_question_count > 0 {
            return Err(ServiceError::InterviewQuestionPending);
        }
        let running_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ai_analyses WHERE activity_id = ? AND status IN ('pending', 'running')",
        )
        .bind(&activity_id)
        .fetch_one(&mut *tx)
        .await?;
        if running_count > 0 {
            return Err(ServiceError::AnalysisAlreadyRunning);
        }
        sqlx::query("INSERT INTO ai_analyses (id, activity_id, status, submitted_payload, provider, model, codex_version, prompt_version, schema_version, created_at) VALUES (?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?)")
            .bind(&id).bind(&activity_id).bind(input.submitted_payload).bind(input.provider).bind(input.model).bind(input.codex_version).bind(input.prompt_version).bind(input.schema_version).bind(utc_now()).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO activity_workflows (activity_id, state, version, updated_at) VALUES (?, 'analysis_running', 1, ?) ON CONFLICT(activity_id) DO UPDATE SET state = 'analysis_running', version = activity_workflows.version + 1, updated_at = excluded.updated_at")
            .bind(activity_id)
            .bind(utc_now())
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(id)
    }

    pub async fn save_analysis_result(
        &self,
        analysis_id: &str,
        raw_result_json: &str,
        candidates: Vec<(String, Option<String>, f64, String, String)>,
        next_question_json: Option<&str>,
        prompt_version: &str,
        schema_version: &str,
    ) -> Result<(), ServiceError> {
        let mut tx = self.db.pool().begin().await?;
        let analysis = sqlx::query("SELECT activity_id FROM ai_analyses WHERE id = ?")
            .bind(analysis_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ServiceError::NotFound("analysis"))?;
        let activity_id: String = analysis.get("activity_id");
        let completed = sqlx::query("UPDATE ai_analyses SET status = 'succeeded', raw_result_json = ?, completed_at = ? WHERE id = ? AND status = 'running'")
            .bind(raw_result_json)
            .bind(utc_now())
            .bind(analysis_id)
            .execute(&mut *tx)
            .await?;
        if completed.rows_affected() != 1 {
            return Err(ServiceError::AnalysisNotRunning);
        }
        let revision: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(revision), 0) + 1 FROM activity_structures WHERE activity_id = ?",
        )
        .bind(&activity_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO activity_structures (id, activity_id, analysis_id, revision, structured_json, source, prompt_version, schema_version, created_at) VALUES (?, ?, ?, ?, ?, 'codex_analysis', ?, ?, ?)")
            .bind(Uuid::new_v4().to_string())
            .bind(&activity_id)
            .bind(analysis_id)
            .bind(revision)
            .bind(raw_result_json)
            .bind(prompt_version)
            .bind(schema_version)
            .bind(utc_now())
            .execute(&mut *tx)
            .await?;
        for (skill_id, specialized_name, confidence, reason, evidence) in candidates {
            if !skill_exists(&mut tx, &skill_id).await? {
                return Err(ServiceError::UnknownSkill(skill_id));
            }
            let specialized_name = validate_specialized_name(specialized_name)?;
            let normalized = specialized_name.as_deref().map(normalize_specialized_name);
            sqlx::query("INSERT INTO skill_candidates (id, analysis_id, skill_id, specialized_skill_name, normalized_specialized_skill_name, confidence, reason, evidence) VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(analysis_id, skill_id) DO UPDATE SET specialized_skill_name = excluded.specialized_skill_name, normalized_specialized_skill_name = excluded.normalized_specialized_skill_name, confidence = excluded.confidence, reason = excluded.reason, evidence = excluded.evidence")
                .bind(Uuid::new_v4().to_string()).bind(analysis_id).bind(skill_id).bind(specialized_name).bind(normalized).bind(confidence).bind(reason).bind(evidence).execute(&mut *tx).await?;
        }
        let workflow_state = if let Some(question_json) = next_question_json {
            sqlx::query("INSERT INTO interview_sessions (id, activity_id, analysis_id, status, current_question_json, prompt_version, schema_version, created_at, updated_at) VALUES (?, ?, ?, 'pending', ?, ?, ?, ?, ?)")
                .bind(Uuid::new_v4().to_string())
                .bind(&activity_id)
                .bind(analysis_id)
                .bind(question_json)
                .bind(prompt_version)
                .bind(schema_version)
                .bind(utc_now())
                .bind(utc_now())
                .execute(&mut *tx)
                .await?;
            "needs_input"
        } else {
            "review_pending"
        };
        sqlx::query("UPDATE activity_workflows SET state = ?, version = version + 1, updated_at = ? WHERE activity_id = ?")
            .bind(workflow_state)
            .bind(utc_now())
            .bind(&activity_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Confirms only explicit accepted candidates. Observation and XP writes are atomic and safe to retry.
    pub async fn confirm_analysis(
        &self,
        analysis_id: &str,
        decisions: Vec<CandidateDecision>,
    ) -> Result<XpSummary, ServiceError> {
        let mut tx = self.db.pool().begin().await?;
        let analysis = sqlx::query("SELECT activity_id, status FROM ai_analyses WHERE id = ?")
            .bind(analysis_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ServiceError::NotFound("analysis"))?;
        let activity_id: String = analysis.get("activity_id");
        let status: String = analysis.get("status");
        if status == "confirmed" {
            let total = total_xp(&mut tx).await?;
            tx.commit().await?;
            return Ok(summary(total));
        }
        if status != "succeeded" {
            return Err(ServiceError::AnalysisNotConfirmable);
        }
        let latest_analysis_id: String = sqlx::query_scalar(
            "SELECT id FROM ai_analyses WHERE activity_id = ? ORDER BY created_at DESC, rowid DESC LIMIT 1",
        )
        .bind(&activity_id)
        .fetch_one(&mut *tx)
        .await?;
        if latest_analysis_id != analysis_id {
            return Err(ServiceError::AnalysisNotConfirmable);
        }
        let workflow_state: Option<String> =
            sqlx::query_scalar("SELECT state FROM activity_workflows WHERE activity_id = ?")
                .bind(&activity_id)
                .fetch_optional(&mut *tx)
                .await?;
        if workflow_state.as_deref() != Some("review_pending") {
            return Err(ServiceError::AnalysisNotConfirmable);
        }
        let candidate_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM skill_candidates WHERE analysis_id = ?")
                .bind(analysis_id)
                .fetch_one(&mut *tx)
                .await?;
        let unique_decisions = decisions
            .iter()
            .map(|decision| decision.candidate_id.as_str())
            .collect::<HashSet<_>>();
        if decisions.len() != candidate_count as usize || unique_decisions.len() != decisions.len()
        {
            return Err(ServiceError::IncompleteCandidateDecisions);
        }
        for decision in decisions {
            let candidate = sqlx::query(
                "SELECT skill_id, specialized_skill_name, reason, evidence FROM skill_candidates WHERE id = ? AND analysis_id = ?",
            )
            .bind(&decision.candidate_id)
            .bind(analysis_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ServiceError::InvalidCandidate(decision.candidate_id.clone()))?;
            let original_skill_id: String = candidate.get("skill_id");
            let is_edited = matches!(decision.decision, CandidateDecisionValue::Edited);
            let edited_reason = if is_edited {
                Some(validate_required_candidate_edit(
                    decision.edited_reason.as_deref(),
                    "理由",
                )?)
            } else {
                None
            };
            let edited_evidence = if is_edited {
                Some(validate_required_candidate_edit(
                    decision.edited_evidence.as_deref(),
                    "証拠",
                )?)
            } else {
                None
            };
            let skill_id = if is_edited {
                decision.edited_skill_id.unwrap_or(original_skill_id)
            } else {
                original_skill_id
            };
            if !skill_exists(&mut tx, &skill_id).await? {
                return Err(ServiceError::UnknownSkill(skill_id));
            }
            let original_specialized_name: Option<String> = candidate.get("specialized_skill_name");
            let specialized_name = validate_specialized_name(if is_edited {
                decision
                    .edited_specialized_skill_name
                    .or(original_specialized_name)
            } else {
                original_specialized_name
            })?;
            let normalized_specialized_name =
                specialized_name.as_deref().map(normalize_specialized_name);
            let duplicate_candidate: Option<String> = sqlx::query_scalar(
                "SELECT id FROM skill_candidates WHERE analysis_id = ? AND skill_id = ? AND id != ? LIMIT 1",
            )
            .bind(analysis_id)
            .bind(&skill_id)
            .bind(&decision.candidate_id)
            .fetch_optional(&mut *tx)
            .await?;
            if duplicate_candidate.is_some() {
                return Err(ServiceError::InvalidCandidate(format!(
                    "canonical skill {skill_id} is already used by another candidate"
                )));
            }
            let candidate_reason: String = candidate.get("reason");
            let candidate_evidence: String = candidate.get("evidence");
            let now = utc_now();
            let new_status = decision.decision.as_str();
            sqlx::query("UPDATE skill_candidates SET skill_id = ?, specialized_skill_name = ?, normalized_specialized_skill_name = ?, decision = ?, edited_reason = ?, edited_evidence = ?, decided_at = ? WHERE id = ?")
                .bind(&skill_id)
                .bind(&specialized_name)
                .bind(&normalized_specialized_name)
                .bind(new_status)
                .bind(edited_reason.as_deref())
                .bind(edited_evidence.as_deref())
                .bind(&now)
                .bind(&decision.candidate_id)
                .execute(&mut *tx)
                .await?;
            if decision.decision.creates_observation() {
                let evidence = if is_edited {
                    edited_evidence.clone().unwrap_or(candidate_evidence)
                } else {
                    candidate_evidence
                };
                let _reason = if is_edited {
                    edited_reason.clone().unwrap_or(candidate_reason)
                } else {
                    candidate_reason
                };
                sqlx::query("INSERT INTO skill_observations (id, activity_id, analysis_id, skill_id, specialized_skill_name, normalized_specialized_skill_name, evidence, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(analysis_id, skill_id) DO NOTHING")
                    .bind(Uuid::new_v4().to_string()).bind(&activity_id).bind(analysis_id).bind(skill_id).bind(specialized_name).bind(normalized_specialized_name).bind(evidence).bind(now).execute(&mut *tx).await?;
            }
        }
        sqlx::query("UPDATE ai_analyses SET status = 'confirmed', confirmed_at = ? WHERE id = ?")
            .bind(utc_now())
            .bind(analysis_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE activity_workflows SET state = 'confirmed', version = version + 1, updated_at = ? WHERE activity_id = ?")
            .bind(utc_now())
            .bind(&activity_id)
            .execute(&mut *tx)
            .await?;
        let reason_key = format!("analysis:{analysis_id}:confirmed");
        self.award_xp(
            &mut tx,
            XpEvent {
                amount: ANALYSIS_XP,
                reason: XpReason::AnalysisConfirmed,
                key: &reason_key,
                activity_id: Some(&activity_id),
                analysis_id: Some(analysis_id),
                quest_id: None,
                description: "AI分析を確認",
            },
        )
        .await?;
        let total = total_xp(&mut tx).await?;
        tx.commit().await?;
        Ok(summary(total))
    }

    pub async fn create_quest(&self, input: NewQuest) -> Result<String, ServiceError> {
        if let Some(skill) = &input.target_skill_id
            && !skill_exists_pool(self.db.pool(), skill).await?
        {
            return Err(ServiceError::UnknownSkill(skill.clone()));
        }
        let id = Uuid::new_v4().to_string();
        let now = utc_now();
        sqlx::query("INSERT INTO quests (id, template_id, title, description, status, target_skill_id, difficulty, estimated_minutes, success_criteria_json, evidence_prompt, scheduled_on, created_at, updated_at) VALUES (?, ?, ?, ?, 'proposed', ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&id).bind(input.template_id).bind(input.title).bind(input.description).bind(input.target_skill_id).bind(input.difficulty).bind(input.estimated_minutes).bind(input.success_criteria_json).bind(input.evidence_prompt).bind(input.scheduled_on).bind(&now).bind(&now).execute(self.db.pool()).await?;
        Ok(id)
    }

    pub async fn create_quest_from_generation(
        &self,
        input: NewQuest,
        run_id: &str,
        raw_result_json: &str,
    ) -> Result<String, ServiceError> {
        let mut tx = self.db.pool().begin().await?;
        if let Some(skill) = &input.target_skill_id
            && !skill_exists(&mut tx, skill).await?
        {
            return Err(ServiceError::UnknownSkill(skill.clone()));
        }
        let id = Uuid::new_v4().to_string();
        let now = utc_now();
        sqlx::query("INSERT INTO quests (id, template_id, title, description, status, target_skill_id, difficulty, estimated_minutes, success_criteria_json, evidence_prompt, scheduled_on, created_at, updated_at) VALUES (?, ?, ?, ?, 'proposed', ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(&id).bind(input.template_id).bind(input.title).bind(input.description).bind(input.target_skill_id).bind(input.difficulty).bind(input.estimated_minutes).bind(input.success_criteria_json).bind(input.evidence_prompt).bind(input.scheduled_on).bind(&now).bind(&now).execute(&mut *tx).await?;
        let completed = sqlx::query("UPDATE quest_generation_runs SET status = 'succeeded', quest_id = ?, raw_result_json = ?, completed_at = ? WHERE id = ? AND status = 'running'")
            .bind(&id)
            .bind(raw_result_json)
            .bind(&now)
            .bind(run_id)
            .execute(&mut *tx)
            .await?;
        if completed.rows_affected() != 1 {
            return Err(ServiceError::QuestGenerationNotRunning);
        }
        tx.commit().await?;
        Ok(id)
    }

    pub async fn transition_quest(
        &self,
        quest_id: &str,
        to: QuestStatus,
    ) -> Result<(), ServiceError> {
        self.transition_quest_with_details(quest_id, to, None, None)
            .await
    }

    pub async fn transition_quest_with_details(
        &self,
        quest_id: &str,
        to: QuestStatus,
        scheduled_on: Option<&str>,
        estimated_minutes: Option<i64>,
    ) -> Result<(), ServiceError> {
        let mut tx = self.db.pool().begin().await?;
        let row = sqlx::query("SELECT status FROM quests WHERE id = ?")
            .bind(quest_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ServiceError::NotFound("quest"))?;
        let from = parse_status(row.get("status"))?;
        from.transition_to(to)
            .map_err(|_| ServiceError::InvalidQuestTransition {
                from: from.as_str().into(),
                to: to.as_str().into(),
            })?;
        sqlx::query("UPDATE quests SET status = ?, scheduled_on = COALESCE(?, scheduled_on), estimated_minutes = COALESCE(?, estimated_minutes), updated_at = ? WHERE id = ?")
            .bind(to.as_str())
            .bind(scheduled_on)
            .bind(estimated_minutes)
            .bind(utc_now())
            .bind(quest_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn save_reflection(
        &self,
        quest_id: &str,
        result: ReflectionResult,
        learned: &str,
        difficulty_actual: Option<i64>,
        next_action: &str,
    ) -> Result<XpSummary, ServiceError> {
        let mut tx = self.db.pool().begin().await?;
        let quest = sqlx::query("SELECT id, status FROM quests WHERE id = ?")
            .bind(quest_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ServiceError::NotFound("quest"))?;
        let quest_id: String = quest.get("id");
        let status: String = quest.get("status");
        if status != QuestStatus::Completed.as_str() {
            return Err(ServiceError::QuestNotReflectable);
        }
        sqlx::query("INSERT INTO quest_reflections (id, quest_id, result, learned, difficulty_actual, next_action, created_at) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(quest_id) DO NOTHING")
            .bind(Uuid::new_v4().to_string()).bind(&quest_id).bind(result.as_str()).bind(learned).bind(difficulty_actual).bind(next_action).bind(utc_now()).execute(&mut *tx).await?;
        let reason_key = format!("quest:{quest_id}:reflection");
        self.award_xp(
            &mut tx,
            XpEvent {
                amount: REFLECTION_XP,
                reason: XpReason::QuestReflectionSaved,
                key: &reason_key,
                activity_id: None,
                analysis_id: None,
                quest_id: Some(&quest_id),
                description: "クエストを振り返り",
            },
        )
        .await?;
        let total = total_xp(&mut tx).await?;
        tx.commit().await?;
        Ok(summary(total))
    }

    async fn award_xp(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        event: XpEvent<'_>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO xp_events (id, amount, reason_type, reason_key, activity_id, analysis_id, quest_id, description, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(reason_key) DO NOTHING")
            .bind(Uuid::new_v4().to_string()).bind(event.amount).bind(event.reason.as_str()).bind(event.key).bind(event.activity_id).bind(event.analysis_id).bind(event.quest_id).bind(event.description).bind(utc_now()).execute(&mut **tx).await?;
        Ok(())
    }
}

fn utc_now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn validate_specialized_name(name: Option<String>) -> Result<Option<String>, ServiceError> {
    let Some(name) = name else { return Ok(None) };
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > 80 {
        return Err(ServiceError::InvalidCandidate(
            "specialized skill name must be at most 80 characters".into(),
        ));
    }
    Ok(Some(trimmed.to_owned()))
}

fn validate_required_candidate_edit(
    value: Option<&str>,
    label: &str,
) -> Result<String, ServiceError> {
    let value = value.map(str::trim).unwrap_or_default();
    if value.is_empty() {
        return Err(ServiceError::InvalidCandidateEdit(format!(
            "編集して採用する場合は{label}を入力してください"
        )));
    }
    if value.chars().count() > 1_000 {
        return Err(ServiceError::InvalidCandidateEdit(format!(
            "{label}は1000文字以内で入力してください"
        )));
    }
    Ok(value.to_owned())
}

fn normalize_specialized_name(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
fn summary(total_xp: i64) -> XpSummary {
    XpSummary {
        total_xp,
        level: level_for_total_xp(total_xp),
    }
}
async fn total_xp(tx: &mut Transaction<'_, Sqlite>) -> Result<i64, sqlx::Error> {
    Ok(
        sqlx::query("SELECT COALESCE(SUM(amount), 0) total FROM xp_events")
            .fetch_one(&mut **tx)
            .await?
            .get("total"),
    )
}
async fn skill_exists(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("SELECT id FROM skills WHERE id = ? AND is_active = 1")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
            .is_some(),
    )
}
async fn skill_exists_pool(pool: &sqlx::SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    Ok(
        sqlx::query("SELECT id FROM skills WHERE id = ? AND is_active = 1")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .is_some(),
    )
}
fn parse_status(value: String) -> Result<QuestStatus, ServiceError> {
    match value.as_str() {
        "proposed" => Ok(QuestStatus::Proposed),
        "accepted" => Ok(QuestStatus::Accepted),
        "in_progress" => Ok(QuestStatus::InProgress),
        "completed" => Ok(QuestStatus::Completed),
        "rescheduled" => Ok(QuestStatus::Rescheduled),
        "adjusted" => Ok(QuestStatus::Adjusted),
        "cancelled" => Ok(QuestStatus::Cancelled),
        _ => Err(ServiceError::NotFound("quest status")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn service() -> (tempfile::NamedTempFile, GrowthService) {
        let file = tempfile::NamedTempFile::new().unwrap();
        let db = Database::open(file.path()).await.unwrap();
        (file, GrowthService::new(db))
    }
    async fn mark_analysis_running(service: &GrowthService, analysis_id: &str) {
        sqlx::query(
            "UPDATE ai_analyses SET status = 'running' WHERE id = ? AND status = 'pending'",
        )
        .bind(analysis_id)
        .execute(service.db().pool())
        .await
        .unwrap();
    }
    #[tokio::test]
    async fn activity_xp_is_recorded_once() {
        let (_file, service) = service().await;
        let activity = service
            .create_activity(NewActivity {
                occurred_on: "2026-07-20".into(),
                action_text: "設計した".into(),
                challenge_text: "".into(),
                outcome_text: "".into(),
            })
            .await
            .unwrap();
        assert!(!activity.is_empty());
        let total: i64 = sqlx::query("SELECT SUM(amount) total FROM xp_events")
            .fetch_one(service.db().pool())
            .await
            .unwrap()
            .get("total");
        assert_eq!(total, 10);
    }

    #[tokio::test]
    async fn quick_capture_saves_compatibility_row_capture_workflow_and_xp_atomically() {
        let (_file, service) = service().await;
        let activity = service
            .quick_capture_activity("2026-07-20", "  障害対応の手順を整理した\n", "quick")
            .await
            .unwrap();
        let saved: (String, String, String, String) = sqlx::query_as(
            "SELECT a.action_text, c.raw_text, c.capture_mode, w.state FROM activities a JOIN activity_captures c ON c.activity_id = a.id JOIN activity_workflows w ON w.activity_id = a.id WHERE a.id = ?",
        )
        .bind(&activity)
        .fetch_one(service.db().pool())
        .await
        .unwrap();
        assert_eq!(
            saved,
            (
                "  障害対応の手順を整理した\n".into(),
                "  障害対応の手順を整理した\n".into(),
                "quick".into(),
                "captured".into(),
            )
        );
        let xp: (i64, String) =
            sqlx::query_as("SELECT amount, reason_key FROM xp_events WHERE activity_id = ?")
                .bind(&activity)
                .fetch_one(service.db().pool())
                .await
                .unwrap();
        assert_eq!(xp, (10, format!("activity:{activity}")));
    }

    #[tokio::test]
    async fn activity_allows_only_one_running_analysis_and_blocks_reanalysis_with_open_question() {
        let (_file, service) = service().await;
        let activity = service
            .quick_capture_activity("2026-07-20", "SQLを速くした", "guided")
            .await
            .unwrap();
        let analysis = service
            .create_analysis(NewAnalysis {
                activity_id: activity.clone(),
                submitted_payload: "{}".into(),
                provider: "test".into(),
                model: None,
                codex_version: None,
                prompt_version: "v2".into(),
                schema_version: "v2".into(),
            })
            .await
            .unwrap();
        assert!(matches!(
            service
                .create_analysis(NewAnalysis {
                    activity_id: activity.clone(),
                    submitted_payload: "{}".into(),
                    provider: "test".into(),
                    model: None,
                    codex_version: None,
                    prompt_version: "v2".into(),
                    schema_version: "v2".into(),
                })
                .await,
            Err(ServiceError::AnalysisAlreadyRunning)
        ));
        mark_analysis_running(&service, &analysis).await;
        service
            .save_analysis_result(
                &analysis,
                "{}",
                vec![],
                Some(r#"{"questionId":"measurement","target":"measurement","text":"どれくらい改善しましたか？","answerType":"text","choices":[],"whyItMatters":"成果を確認するため"}"#),
                "v2",
                "v2",
            )
            .await
            .unwrap();
        assert!(matches!(
            service
                .create_analysis(NewAnalysis {
                    activity_id: activity,
                    submitted_payload: "{}".into(),
                    provider: "test".into(),
                    model: None,
                    codex_version: None,
                    prompt_version: "v2".into(),
                    schema_version: "v2".into(),
                })
                .await,
            Err(ServiceError::InterviewQuestionPending)
        ));
    }

    #[tokio::test]
    async fn cancelled_analysis_cannot_publish_results_or_derived_rows() {
        let (_file, service) = service().await;
        let activity = service
            .quick_capture_activity("2026-07-20", "SQLを速くした", "quick")
            .await
            .unwrap();
        let analysis = service
            .create_analysis(NewAnalysis {
                activity_id: activity,
                submitted_payload: "{}".into(),
                provider: "test".into(),
                model: None,
                codex_version: None,
                prompt_version: "v2".into(),
                schema_version: "v2".into(),
            })
            .await
            .unwrap();
        mark_analysis_running(&service, &analysis).await;
        sqlx::query("UPDATE ai_analyses SET status = 'cancelled' WHERE id = ?")
            .bind(&analysis)
            .execute(service.db().pool())
            .await
            .unwrap();
        assert!(matches!(
            service
                .save_analysis_result(
                    &analysis,
                    r#"{"summary":"late"}"#,
                    vec![(
                        "technical.validation".into(),
                        None,
                        0.8,
                        "reason".into(),
                        "evidence".into(),
                    )],
                    None,
                    "v2",
                    "v2",
                )
                .await,
            Err(ServiceError::AnalysisNotRunning)
        ));
        let status: String = sqlx::query_scalar("SELECT status FROM ai_analyses WHERE id = ?")
            .bind(&analysis)
            .fetch_one(service.db().pool())
            .await
            .unwrap();
        let derived: i64 = sqlx::query_scalar(
            "SELECT (SELECT COUNT(*) FROM activity_structures WHERE analysis_id = ?) + (SELECT COUNT(*) FROM skill_candidates WHERE analysis_id = ?) + (SELECT COUNT(*) FROM interview_sessions WHERE analysis_id = ?)",
        )
        .bind(&analysis)
        .bind(&analysis)
        .bind(&analysis)
        .fetch_one(service.db().pool())
        .await
        .unwrap();
        assert_eq!(status, "cancelled");
        assert_eq!(derived, 0);
    }

    #[tokio::test]
    async fn interview_question_blocks_confirmation_without_creating_observation_or_xp() {
        let (_file, service) = service().await;
        let activity = service
            .quick_capture_activity("2026-07-20", "SQLを速くした", "guided")
            .await
            .unwrap();
        let analysis = service
            .create_analysis(NewAnalysis {
                activity_id: activity.clone(),
                submitted_payload: "{}".into(),
                provider: "test".into(),
                model: None,
                codex_version: None,
                prompt_version: "v2".into(),
                schema_version: "v2".into(),
            })
            .await
            .unwrap();
        let question = r#"{"questionId":"measurement","target":"measurement","prompt":"どれくらい改善しましたか？","answerType":"text","choices":[]}"#;
        mark_analysis_running(&service, &analysis).await;
        service
            .save_analysis_result(
                &analysis,
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
        let candidate: String =
            sqlx::query_scalar("SELECT id FROM skill_candidates WHERE analysis_id = ?")
                .bind(&analysis)
                .fetch_one(service.db().pool())
                .await
                .unwrap();
        assert!(matches!(
            service
                .confirm_analysis(
                    &analysis,
                    vec![CandidateDecision {
                        candidate_id: candidate,
                        decision: CandidateDecisionValue::Accepted,
                        edited_reason: None,
                        edited_evidence: None,
                        edited_skill_id: None,
                        edited_specialized_skill_name: None,
                    }],
                )
                .await,
            Err(ServiceError::AnalysisNotConfirmable)
        ));
        let workflow: String =
            sqlx::query_scalar("SELECT state FROM activity_workflows WHERE activity_id = ?")
                .bind(&activity)
                .fetch_one(service.db().pool())
                .await
                .unwrap();
        let structure_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM activity_structures WHERE activity_id = ?")
                .bind(&activity)
                .fetch_one(service.db().pool())
                .await
                .unwrap();
        let session_status: String =
            sqlx::query_scalar("SELECT status FROM interview_sessions WHERE analysis_id = ?")
                .bind(&analysis)
                .fetch_one(service.db().pool())
                .await
                .unwrap();
        let observation_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM skill_observations WHERE analysis_id = ?")
                .bind(&analysis)
                .fetch_one(service.db().pool())
                .await
                .unwrap();
        let xp: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(amount), 0) FROM xp_events")
            .fetch_one(service.db().pool())
            .await
            .unwrap();
        assert_eq!(workflow, "needs_input");
        assert_eq!(structure_count, 1);
        assert_eq!(session_status, "pending");
        assert_eq!(observation_count, 0);
        assert_eq!(xp, 10);
    }

    #[tokio::test]
    async fn confirmation_creates_only_approved_evidence_and_is_idempotent() {
        let (_file, service) = service().await;
        let activity = service
            .create_activity(NewActivity {
                occurred_on: "2026-07-20".into(),
                action_text: "".into(),
                challenge_text: "".into(),
                outcome_text: "".into(),
            })
            .await
            .unwrap();
        let analysis = service
            .create_analysis(NewAnalysis {
                activity_id: activity,
                submitted_payload: "{}".into(),
                provider: "test".into(),
                model: None,
                codex_version: None,
                prompt_version: "v1".into(),
                schema_version: "v1".into(),
            })
            .await
            .unwrap();
        mark_analysis_running(&service, &analysis).await;
        service
            .save_analysis_result(
                &analysis,
                "{}",
                vec![(
                    "communication.explanation".into(),
                    Some("SQL性能分析".into()),
                    0.8,
                    "reason".into(),
                    "evidence".into(),
                )],
                None,
                "v2",
                "v2",
            )
            .await
            .unwrap();
        let before_confirmation_xp: i64 =
            sqlx::query_scalar("SELECT COALESCE(SUM(amount), 0) FROM xp_events")
                .fetch_one(service.db().pool())
                .await
                .unwrap();
        let before_confirmation_observations: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM skill_observations")
                .fetch_one(service.db().pool())
                .await
                .unwrap();
        let raw_activity: (String, String, String) = sqlx::query_as(
            "SELECT action_text, challenge_text, outcome_text FROM activities LIMIT 1",
        )
        .fetch_one(service.db().pool())
        .await
        .unwrap();
        assert_eq!(before_confirmation_xp, 10);
        assert_eq!(before_confirmation_observations, 0);
        assert_eq!(raw_activity, (String::new(), String::new(), String::new()));
        let candidate: String =
            sqlx::query("SELECT id FROM skill_candidates WHERE analysis_id = ?")
                .bind(&analysis)
                .fetch_one(service.db().pool())
                .await
                .unwrap()
                .get("id");
        assert!(matches!(
            service
                .confirm_analysis(
                    &analysis,
                    vec![CandidateDecision {
                        candidate_id: candidate.clone(),
                        decision: CandidateDecisionValue::Edited,
                        edited_reason: Some(" ".into()),
                        edited_evidence: Some(String::new()),
                        edited_skill_id: Some("thinking.problem_decomposition".into()),
                        edited_specialized_skill_name: None,
                    }],
                )
                .await,
            Err(ServiceError::InvalidCandidateEdit(_))
        ));
        let first = service
            .confirm_analysis(
                &analysis,
                vec![CandidateDecision {
                    candidate_id: candidate.clone(),
                    decision: CandidateDecisionValue::Edited,
                    edited_reason: Some("問題を分解した".into()),
                    edited_evidence: Some("SQLの原因候補を切り分けた".into()),
                    edited_skill_id: Some("thinking.problem_decomposition".into()),
                    edited_specialized_skill_name: Some(" SQL   性能調査 ".into()),
                }],
            )
            .await
            .unwrap();
        let second = service.confirm_analysis(&analysis, vec![]).await.unwrap();
        assert_eq!(first.total_xp, 30);
        assert_eq!(second.total_xp, 30);
        let observations: i64 = sqlx::query("SELECT COUNT(*) count FROM skill_observations")
            .fetch_one(service.db().pool())
            .await
            .unwrap()
            .get("count");
        assert_eq!(observations, 1);
        let observation: (String, String, String) = sqlx::query_as(
            "SELECT skill_id, specialized_skill_name, normalized_specialized_skill_name FROM skill_observations WHERE analysis_id = ?",
        )
        .bind(&analysis)
        .fetch_one(service.db().pool())
        .await
        .unwrap();
        assert_eq!(
            observation,
            (
                "thinking.problem_decomposition".into(),
                "SQL   性能調査".into(),
                "sql 性能調査".into(),
            )
        );
        let raw_activity_after: (String, String, String) = sqlx::query_as(
            "SELECT action_text, challenge_text, outcome_text FROM activities LIMIT 1",
        )
        .fetch_one(service.db().pool())
        .await
        .unwrap();
        assert_eq!(raw_activity_after, raw_activity);
    }
    #[tokio::test]
    async fn reflection_xp_is_idempotent() {
        let (_file, service) = service().await;
        let quest = service
            .create_quest(NewQuest {
                template_id: "clarify.v1".into(),
                title: "確認する".into(),
                description: "短く確認".into(),
                target_skill_id: Some("communication.clarification".into()),
                difficulty: 1,
                estimated_minutes: 10,
                success_criteria_json: "[]".into(),
                evidence_prompt: "結果".into(),
                scheduled_on: None,
            })
            .await
            .unwrap();
        assert!(matches!(
            service
                .save_reflection(&quest, ReflectionResult::Rested, "", None, "")
                .await,
            Err(ServiceError::QuestNotReflectable)
        ));
        service
            .transition_quest(&quest, QuestStatus::Accepted)
            .await
            .unwrap();
        service
            .transition_quest(&quest, QuestStatus::InProgress)
            .await
            .unwrap();
        service
            .transition_quest(&quest, QuestStatus::Completed)
            .await
            .unwrap();
        let one = service
            .save_reflection(&quest, ReflectionResult::Rested, "", None, "")
            .await
            .unwrap();
        let two = service
            .save_reflection(&quest, ReflectionResult::Rested, "", None, "")
            .await
            .unwrap();
        assert_eq!(one.total_xp, 40);
        assert_eq!(two.total_xp, 40);
    }

    #[tokio::test]
    async fn quest_and_generation_run_complete_in_one_transaction() {
        let (_file, service) = service().await;
        let now = utc_now();
        let activity_id = service
            .quick_capture_activity("2026-07-20", "確認事項を整理した", "quick")
            .await
            .unwrap();
        let analysis_id = service
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
        for (run_id, status) in [("run-ok", "running"), ("run-stale", "failed")] {
            sqlx::query("INSERT INTO quest_generation_runs (id, activity_id, analysis_id, status, submitted_payload, provider, prompt_version, schema_version, created_at) VALUES (?, ?, ?, ?, '{}', 'test', 'v1', 'v1', ?)")
                .bind(run_id)
                .bind(&activity_id)
                .bind(&analysis_id)
                .bind(status)
                .bind(&now)
                .execute(service.db().pool())
                .await
                .unwrap();
        }
        let input = || NewQuest {
            template_id: "clarify_once".into(),
            title: "確認する".into(),
            description: "短く確認".into(),
            target_skill_id: Some("communication.clarification".into()),
            difficulty: 1,
            estimated_minutes: 10,
            success_criteria_json: r#"["確認した"]"#.into(),
            evidence_prompt: "結果".into(),
            scheduled_on: None,
        };
        let quest_id = service
            .create_quest_from_generation(input(), "run-ok", r#"{"title":"確認する"}"#)
            .await
            .unwrap();
        let completed: (String, String, String) = sqlx::query_as(
            "SELECT status, quest_id, raw_result_json FROM quest_generation_runs WHERE id = 'run-ok'",
        )
        .fetch_one(service.db().pool())
        .await
        .unwrap();
        assert_eq!(completed.0, "succeeded");
        assert_eq!(completed.1, quest_id);
        assert_eq!(completed.2, r#"{"title":"確認する"}"#);

        assert!(matches!(
            service
                .create_quest_from_generation(input(), "run-stale", "{}")
                .await,
            Err(ServiceError::QuestGenerationNotRunning)
        ));
        let quest_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM quests")
            .fetch_one(service.db().pool())
            .await
            .unwrap();
        assert_eq!(quest_count, 1, "failed run must roll back quest insert");
    }

    #[tokio::test]
    async fn rejected_transition_does_not_partially_update_quest_details() {
        let (_file, service) = service().await;
        let quest = service
            .create_quest(NewQuest {
                template_id: "clarify_once".into(),
                title: "確認する".into(),
                description: "短く確認".into(),
                target_skill_id: Some("communication.clarification".into()),
                difficulty: 1,
                estimated_minutes: 10,
                success_criteria_json: "[]".into(),
                evidence_prompt: "結果".into(),
                scheduled_on: None,
            })
            .await
            .unwrap();
        assert!(
            service
                .transition_quest_with_details(
                    &quest,
                    QuestStatus::Completed,
                    Some("2030-01-01"),
                    Some(5),
                )
                .await
                .is_err()
        );
        let row =
            sqlx::query("SELECT status, scheduled_on, estimated_minutes FROM quests WHERE id = ?")
                .bind(quest)
                .fetch_one(service.db().pool())
                .await
                .unwrap();
        assert_eq!(row.get::<String, _>("status"), "proposed");
        assert_eq!(row.get::<Option<String>, _>("scheduled_on"), None);
        assert_eq!(row.get::<i64, _>("estimated_minutes"), 10);
    }
}
