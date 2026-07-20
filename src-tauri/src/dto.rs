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

pub const USER_PROFILE_SCHEMA_VERSION: i64 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FocusThemeDto {
    pub id: String,
    pub title: String,
    pub desired_outcome: String,
    pub why_now: String,
    pub horizon: String,
    pub status: String,
    pub linked_skill_ids: Vec<String>,
    pub sort_order: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FocusThemeInput {
    pub id: Option<String>,
    pub title: String,
    pub desired_outcome: String,
    pub why_now: String,
    pub horizon: String,
    pub status: String,
    pub linked_skill_ids: Vec<String>,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SaveFocusThemesInput {
    pub themes: Vec<FocusThemeInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserProfileDto {
    pub schema_version: i64,
    pub revision: i64,
    pub role: String,
    pub background: String,
    pub current_responsibilities: String,
    pub domains_and_technologies: Vec<String>,
    pub growth_goal: String,
    pub motivation: String,
    pub current_challenges: String,
    pub recent_success: String,
    pub focus_skill_ids: Vec<String>,
    pub weekly_minutes: i64,
    pub preferred_quest_minutes: i64,
    pub preferred_quest_style: String,
    pub constraints: String,
    pub excluded_quest_patterns: String,
    pub focus_themes: Vec<FocusThemeDto>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateUserProfileInput {
    pub expected_revision: Option<i64>,
    pub role: String,
    pub background: String,
    pub current_responsibilities: String,
    pub domains_and_technologies: Vec<String>,
    pub growth_goal: String,
    pub motivation: String,
    pub current_challenges: String,
    pub recent_success: String,
    pub focus_skill_ids: Vec<String>,
    pub weekly_minutes: i64,
    pub preferred_quest_minutes: i64,
    pub preferred_quest_style: String,
    pub constraints: String,
    pub excluded_quest_patterns: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodexPathCandidateDto {
    pub discovered_path: String,
    pub canonical_path: String,
    pub source: String,
    pub executable: bool,
    pub recommended: bool,
    pub connection: Option<CodexConnectionStatus>,
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
pub struct QuickCaptureInput {
    pub occurred_on: String,
    pub raw_text: String,
    pub capture_mode: String,
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
    #[serde(default)]
    pub edited_skill_id: Option<String>,
    #[serde(default)]
    pub edited_specialized_skill_name: Option<String>,
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
    #[serde(default)]
    pub submitted_payload: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InterviewChoiceDto {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InterviewQuestionDto {
    pub session_id: String,
    pub question_id: String,
    pub target: String,
    pub text: String,
    pub answer_type: String,
    pub choices: Vec<InterviewChoiceDto>,
    pub why_it_matters: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InterviewAnswerInput {
    pub session_id: String,
    pub question_id: String,
    pub answer_state: String,
    pub answer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityWorkflowDto {
    pub activity_id: String,
    pub state: String,
    pub version: i64,
    pub current_question: Option<InterviewQuestionDto>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityInboxItemDto {
    #[serde(flatten)]
    pub activity: ActivityDto,
    pub workflow: ActivityWorkflowDto,
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
pub struct SubmissionPreview {
    pub entity_id: String,
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
    pub specialized_skill_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityAnalysisDto {
    pub id: String,
    pub activity_id: String,
    pub status: String,
    pub summary: Option<String>,
    pub outcomes: Vec<String>,
    pub confirmed_facts: Vec<String>,
    pub unconfirmed_facts: Vec<String>,
    pub skill_candidates: Vec<SkillCandidateDto>,
    pub missing_information_question: Option<String>,
    pub next_question: Option<InterviewQuestionDto>,
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
    pub specialized_skills: Vec<SpecializedSkillSummaryDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpecializedSkillSummaryDto {
    pub name: String,
    pub evidence_count: i64,
    pub last_observed_at: Option<String>,
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillCandidateOutput {
    pub skill_id: String,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub specialized_skill_name: Option<String>,
    pub confidence: f64,
    pub reason: String,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InterviewChoiceOutput {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NextQuestionOutput {
    pub question_id: String,
    pub target: String,
    pub text: String,
    pub answer_type: String,
    pub choices: Vec<InterviewChoiceOutput>,
    pub why_it_matters: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityAnalysisOutput {
    pub summary: String,
    pub outcomes: Vec<String>,
    pub confirmed_facts: Vec<String>,
    pub unconfirmed_facts: Vec<String>,
    pub skill_candidates: Vec<SkillCandidateOutput>,
    #[serde(default)]
    pub missing_information_question: Option<String>,
    #[serde(deserialize_with = "deserialize_required_option")]
    pub next_question: Option<NextQuestionOutput>,
}

fn deserialize_required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct LegacyActivityAnalysisOutput {
    summary: String,
    outcomes: Vec<String>,
    skill_candidates: Vec<serde_json::Value>,
    #[serde(default)]
    missing_information_question: Option<String>,
}

pub fn parse_activity_analysis_output_compat(
    json: &str,
) -> Result<ActivityAnalysisOutput, serde_json::Error> {
    match serde_json::from_str::<ActivityAnalysisOutput>(json) {
        Ok(output) => Ok(output),
        Err(v2_error) => match serde_json::from_str::<LegacyActivityAnalysisOutput>(json) {
            Ok(legacy) => {
                let _ = legacy.skill_candidates;
                Ok(ActivityAnalysisOutput {
                    summary: legacy.summary,
                    outcomes: legacy.outcomes,
                    confirmed_facts: Vec::new(),
                    unconfirmed_facts: Vec::new(),
                    skill_candidates: Vec::new(),
                    missing_information_question: legacy.missing_information_question,
                    next_question: None,
                })
            }
            Err(_) => Err(v2_error),
        },
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
