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
    #[error("candidate {0} does not belong to analysis")]
    InvalidCandidate(String),
    #[error("every analysis candidate must have exactly one decision")]
    IncompleteCandidateDecisions,
    #[error("skill {0} is not in the fixed catalog")]
    UnknownSkill(String),
    #[error("invalid quest transition from {from} to {to}")]
    InvalidQuestTransition { from: String, to: String },
    #[error("quest must be completed before reflection")]
    QuestNotReflectable,
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
        sqlx::query("INSERT INTO ai_analyses (id, activity_id, status, submitted_payload, provider, model, codex_version, prompt_version, schema_version, created_at) VALUES (?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?)")
            .bind(&id).bind(input.activity_id).bind(input.submitted_payload).bind(input.provider).bind(input.model).bind(input.codex_version).bind(input.prompt_version).bind(input.schema_version).bind(utc_now()).execute(self.db.pool()).await?;
        Ok(id)
    }

    pub async fn save_analysis_result(
        &self,
        analysis_id: &str,
        raw_result_json: &str,
        candidates: Vec<(String, f64, String, String)>,
    ) -> Result<(), ServiceError> {
        let mut tx = self.db.pool().begin().await?;
        let exists = sqlx::query("SELECT id FROM ai_analyses WHERE id = ?")
            .bind(analysis_id)
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
        if !exists {
            return Err(ServiceError::NotFound("analysis"));
        }
        sqlx::query("UPDATE ai_analyses SET status = 'succeeded', raw_result_json = ?, completed_at = ? WHERE id = ?") .bind(raw_result_json).bind(utc_now()).bind(analysis_id).execute(&mut *tx).await?;
        for (skill_id, confidence, reason, evidence) in candidates {
            if !skill_exists(&mut tx, &skill_id).await? {
                return Err(ServiceError::UnknownSkill(skill_id));
            }
            sqlx::query("INSERT INTO skill_candidates (id, analysis_id, skill_id, confidence, reason, evidence) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(analysis_id, skill_id) DO UPDATE SET confidence = excluded.confidence, reason = excluded.reason, evidence = excluded.evidence")
                .bind(Uuid::new_v4().to_string()).bind(analysis_id).bind(skill_id).bind(confidence).bind(reason).bind(evidence).execute(&mut *tx).await?;
        }
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
                "SELECT skill_id, reason, evidence FROM skill_candidates WHERE id = ? AND analysis_id = ?",
            )
            .bind(&decision.candidate_id)
            .bind(analysis_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ServiceError::InvalidCandidate(decision.candidate_id.clone()))?;
            let skill_id: String = candidate.get("skill_id");
            let candidate_reason: String = candidate.get("reason");
            let candidate_evidence: String = candidate.get("evidence");
            let now = utc_now();
            let new_status = decision.decision.as_str();
            let edited_reason = decision.edited_reason.as_deref();
            let edited_evidence = decision.edited_evidence.as_deref();
            sqlx::query("UPDATE skill_candidates SET decision = ?, edited_reason = ?, edited_evidence = ?, decided_at = ? WHERE id = ?")
                .bind(new_status)
                .bind(edited_reason)
                .bind(edited_evidence)
                .bind(&now)
                .bind(&decision.candidate_id)
                .execute(&mut *tx)
                .await?;
            if decision.decision.creates_observation() {
                let evidence = decision.edited_evidence.unwrap_or(candidate_evidence);
                let _reason = decision.edited_reason.unwrap_or(candidate_reason);
                sqlx::query("INSERT INTO skill_observations (id, activity_id, analysis_id, skill_id, evidence, created_at) VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(analysis_id, skill_id) DO NOTHING")
                    .bind(Uuid::new_v4().to_string()).bind(&activity_id).bind(analysis_id).bind(skill_id).bind(evidence).bind(now).execute(&mut *tx).await?;
            }
        }
        sqlx::query("UPDATE ai_analyses SET status = 'confirmed', confirmed_at = ? WHERE id = ?")
            .bind(utc_now())
            .bind(analysis_id)
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
        service
            .save_analysis_result(
                &analysis,
                "{}",
                vec![(
                    "communication.explanation".into(),
                    0.8,
                    "reason".into(),
                    "evidence".into(),
                )],
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
        let first = service
            .confirm_analysis(
                &analysis,
                vec![CandidateDecision {
                    candidate_id: candidate.clone(),
                    decision: CandidateDecisionValue::Accepted,
                    edited_reason: None,
                    edited_evidence: None,
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
