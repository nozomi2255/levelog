use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BootState {
    pub onboarding_complete: bool,
    pub codex: Option<CodexConnectionStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodexConnectionStatus {
    pub available: bool,
    pub authenticated: bool,
    pub path: String,
    pub version: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodexConnectionInput {
    pub codex_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingInput {
    pub role: String,
    pub focus_skill_ids: Vec<String>,
    pub weekly_minutes: i64,
    pub excluded_quest_patterns: String,
    pub codex_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppSettingsDto {
    pub role: String,
    pub focus_skill_ids: Vec<String>,
    pub weekly_minutes: i64,
    pub excluded_quest_patterns: String,
    pub codex_path: String,
    pub onboarding_complete: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateActivityInput {
    pub occurred_on: String,
    pub action_text: String,
    pub challenge_text: String,
    pub outcome_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StartAnalysisInput {
    pub activity_id: String,
    pub submitted_payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CandidateDecisionInput {
    pub candidate_id: String,
    pub decision: String,
    pub edited_reason: Option<String>,
    pub edited_evidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmAnalysisInput {
    pub analysis_id: String,
    pub candidate_decisions: Vec<CandidateDecisionInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GenerateQuestInput {
    pub activity_id: String,
    pub analysis_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuestTransitionInput {
    pub quest_id: String,
    pub action: String,
    pub scheduled_on: Option<String>,
    pub estimated_minutes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuestReflectionInput {
    pub quest_id: String,
    pub result: String,
    pub learned: String,
    pub difficulty_actual: Option<i64>,
    pub next_action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityDto {
    pub id: String,
    pub occurred_on: String,
    pub action_text: String,
    pub challenge_text: String,
    pub outcome_text: String,
    pub created_at: String,
    pub analysis_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityDetailDto {
    #[serde(flatten)]
    pub activity: ActivityDto,
    pub analyses: Vec<ActivityAnalysisDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisPreview {
    pub activity_id: String,
    pub submitted_payload: String,
    pub cloud_inference_notice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisJobDto {
    pub id: String,
    pub activity_id: String,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SkillCandidateDto {
    pub id: String,
    pub skill_id: String,
    pub confidence: f64,
    pub reason: String,
    pub evidence: String,
    pub decision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityAnalysisDto {
    pub id: String,
    pub activity_id: String,
    pub status: String,
    pub summary: Option<String>,
    pub outcomes: Vec<String>,
    pub skill_candidates: Vec<SkillCandidateDto>,
    pub missing_information_question: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmAnalysisResult {
    pub analysis_id: String,
    pub confirmed_observation_count: i64,
    pub xp_awarded: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuestDto {
    pub id: String,
    pub template_id: String,
    pub title: String,
    pub description: String,
    pub target_skill_id: String,
    pub estimated_minutes: i64,
    pub difficulty: i64,
    pub success_criteria: Vec<String>,
    pub evidence_prompt: String,
    pub status: String,
    pub scheduled_on: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuestReflectionDto {
    pub id: String,
    pub quest_id: String,
    pub result: String,
    pub learned: String,
    pub difficulty_actual: Option<i64>,
    pub next_action: String,
    pub created_at: String,
    pub xp_awarded: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SkillDto {
    pub id: String,
    pub code: String,
    pub name: String,
    pub category: String,
    pub evidence_count: i64,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyXpPoint {
    pub date: String,
    pub xp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CategoryObservation {
    pub category: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    pub level: i64,
    pub total_xp: i64,
    pub xp_to_next_level: i64,
    pub today_xp: i64,
    pub today_activities: i64,
    pub today_observations: i64,
    pub active_quest: Option<QuestDto>,
    pub recent_activities: Vec<ActivityDto>,
    pub weekly_xp: Vec<WeeklyXpPoint>,
    pub category_observations: Vec<CategoryObservation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BackupResult {
    pub path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub path: String,
    pub schema_version: i64,
    pub exported_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SkillCandidateOutput {
    pub skill_id: String,
    pub confidence: f64,
    pub reason: String,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityAnalysisOutput {
    pub summary: String,
    pub outcomes: Vec<String>,
    pub skill_candidates: Vec<SkillCandidateOutput>,
    pub missing_information_question: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuestProposalOutput {
    pub template_id: String,
    pub title: String,
    pub description: String,
    pub target_skill_id: String,
    pub estimated_minutes: i64,
    pub difficulty: i64,
    pub success_criteria: Vec<String>,
    pub evidence_prompt: String,
}
