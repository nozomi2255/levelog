import { Channel } from "@tauri-apps/api/core";
import type {
  ActivityAnalysisDto,
  ActivityDetailDto,
  ActivityDto,
  ActivityInboxItemDto,
  ActivityWorkflowDto,
  AnalysisJobDto,
  AnalysisPreview,
  AppSettingsDto,
  BackupResult,
  BootState,
  CodexConnectionStatus,
  CodexPathCandidateDto,
  ConfirmAnalysisInput,
  ConfirmAnalysisResult,
  CreateActivityInput,
  DashboardSnapshot,
  ExportResult,
  ClaimActivityLinkInput,
  CreateEvidenceClaimInput,
  CreateEvidenceRelationInput,
  CreatePortfolioDraftInput,
  CreateProjectInput,
  EvidenceAnalysisJobDto,
  EvidenceAnalysisPreviewDto,
  EvidenceClaimDto,
  EvidenceRelationDto,
  EvidenceLibraryDto,
  EvidenceLibraryQuery,
  FocusThemeDto,
  FocusThemeInput,
  GenerateQuestInput,
  OnboardingInput,
  InterviewAnswerInput,
  ImportPastedSourceInput,
  QuickCaptureInput,
  QuestDto,
  QuestReflectionDto,
  QuestReflectionInput,
  QuestTransitionInput,
  PortfolioDraftDto,
  ProjectDetailDto,
  ProjectDto,
  SkillDto,
  StartAnalysisInput,
  SubmissionPreview,
  SourceDocumentDetailDto,
  SourceImportResult,
  StartEvidenceAnalysisInput,
  UpdatePortfolioDraftInput,
  UpdateUserProfileInput,
  UserProfileDto,
  AppUpdateDto,
  AppUpdateEvent,
  ReleaseInfoDto,
} from "./types";
import { invokeCommand } from "./tauri";

export const api = {
  getBootState: () => invokeCommand<BootState>("get_boot_state"),
  saveOnboarding: (input: OnboardingInput) =>
    invokeCommand<AppSettingsDto>("save_onboarding", { input }),
  updateCodexPath: (codexPath: string) =>
    invokeCommand<AppSettingsDto>("update_codex_path", { input: { codexPath } }),
  testCodexConnection: (codexPath: string) =>
    invokeCommand<CodexConnectionStatus>("test_codex_connection", { input: { codexPath } }),
  discoverCodexCandidates: () =>
    invokeCommand<CodexPathCandidateDto[]>("discover_codex_candidates"),
  getUserProfile: () => invokeCommand<UserProfileDto>("get_user_profile"),
  updateUserProfile: (input: UpdateUserProfileInput) =>
    invokeCommand<UserProfileDto>("update_user_profile", { input }),
  listFocusThemes: () => invokeCommand<FocusThemeDto[]>("list_focus_themes"),
  saveFocusThemes: (themes: FocusThemeInput[]) =>
    invokeCommand<FocusThemeDto[]>("save_focus_themes", { input: { themes } }),
  getDashboard: () => invokeCommand<DashboardSnapshot>("get_dashboard"),
  createActivity: (input: CreateActivityInput) =>
    invokeCommand<ActivityDto>("create_activity", { input }),
  quickCaptureActivity: (input: QuickCaptureInput) =>
    invokeCommand<ActivityDto>("quick_capture_activity", { input }),
  listActivities: () => invokeCommand<ActivityDto[]>("list_activities"),
  listActivityInbox: () => invokeCommand<ActivityInboxItemDto[]>("list_activity_inbox"),
  getActivity: (activityId: string) =>
    invokeCommand<ActivityDetailDto>("get_activity", { activityId }),
  getAnalysisPreview: (activityId: string) =>
    invokeCommand<AnalysisPreview>("get_analysis_preview", { activityId }),
  getActivityWorkflow: (activityId: string) =>
    invokeCommand<ActivityWorkflowDto>("get_activity_workflow", { activityId }),
  answerActivityQuestion: (input: InterviewAnswerInput) =>
    invokeCommand<ActivityWorkflowDto>("answer_activity_question", { input }),
  startActivityAnalysis: (input: StartAnalysisInput) =>
    invokeCommand<AnalysisJobDto>("start_activity_analysis", { input }),
  getActivityAnalysis: (analysisId: string) =>
    invokeCommand<ActivityAnalysisDto>("get_activity_analysis", { analysisId }),
  cancelActivityAnalysis: (analysisId: string) =>
    invokeCommand<AnalysisJobDto>("cancel_activity_analysis", { analysisId }),
  confirmActivityAnalysis: (input: ConfirmAnalysisInput) =>
    invokeCommand<ConfirmAnalysisResult>("confirm_activity_analysis", { input }),
  generateQuest: (input: GenerateQuestInput) =>
    invokeCommand<QuestDto>("generate_quest", { input }),
  getQuestPreview: (input: GenerateQuestInput) =>
    invokeCommand<SubmissionPreview>("get_quest_preview", { input }),
  listQuests: () => invokeCommand<QuestDto[]>("list_quests"),
  transitionQuest: (input: QuestTransitionInput) =>
    invokeCommand<QuestDto>("transition_quest", { input }),
  saveQuestReflection: (input: QuestReflectionInput) =>
    invokeCommand<QuestReflectionDto>("save_quest_reflection", { input }),
  listSkills: () => invokeCommand<SkillDto[]>("list_skills"),
  importPastedSource: (input: ImportPastedSourceInput) =>
    invokeCommand<SourceImportResult>("import_pasted_source", { input }),
  pickAndImportSources: () => invokeCommand<SourceImportResult>("pick_and_import_sources"),
  listEvidenceLibrary: (input: EvidenceLibraryQuery) =>
    invokeCommand<EvidenceLibraryDto>("list_evidence_library", { input }),
  getEvidenceSource: (sourceId: string) =>
    invokeCommand<SourceDocumentDetailDto>("get_evidence_source", { sourceId }),
  createEvidenceClaim: (input: CreateEvidenceClaimInput) =>
    invokeCommand<EvidenceClaimDto>("create_evidence_claim", { input }),
  reviewEvidenceClaim: (input: import("./types").ReviewEvidenceClaimInput) =>
    invokeCommand<EvidenceClaimDto>("review_evidence_claim", { input }),
  listEvidenceRelations: () => invokeCommand<EvidenceRelationDto[]>("list_evidence_relations"),
  createEvidenceRelation: (input: CreateEvidenceRelationInput) =>
    invokeCommand<EvidenceRelationDto>("create_evidence_relation", { input }),
  deleteEvidenceRelation: (relationId: string) =>
    invokeCommand<void>("delete_evidence_relation", { relationId }),
  linkClaimToActivity: (input: ClaimActivityLinkInput) =>
    invokeCommand<EvidenceClaimDto>("link_claim_to_activity", { input }),
  createProject: (input: CreateProjectInput) =>
    invokeCommand<ProjectDto>("create_project", { input }),
  listProjects: () => invokeCommand<ProjectDto[]>("list_projects"),
  getProject: (projectId: string) =>
    invokeCommand<ProjectDetailDto>("get_project", { projectId }),
  linkClaimToProject: (projectId: string, claimId: string) =>
    invokeCommand<ProjectDetailDto>("link_claim_to_project", { input: { projectId, claimId } }),
  unlinkClaimFromProject: (projectId: string, claimId: string) =>
    invokeCommand<ProjectDetailDto>("unlink_claim_from_project", { input: { projectId, claimId } }),
  createPortfolioDraft: (input: CreatePortfolioDraftInput) =>
    invokeCommand<PortfolioDraftDto>("create_portfolio_draft", { input }),
  updatePortfolioDraft: (input: UpdatePortfolioDraftInput) =>
    invokeCommand<PortfolioDraftDto>("update_portfolio_draft", { input }),
  listPortfolioDrafts: () => invokeCommand<PortfolioDraftDto[]>("list_portfolio_drafts"),
  getEvidenceAnalysisPreview: (sourceId: string) =>
    invokeCommand<EvidenceAnalysisPreviewDto>("get_evidence_analysis_preview", { sourceId }),
  startEvidenceAnalysis: (input: StartEvidenceAnalysisInput) =>
    invokeCommand<EvidenceAnalysisJobDto>("start_evidence_analysis", { input }),
  getEvidenceAnalysis: (jobId: string) =>
    invokeCommand<EvidenceAnalysisJobDto>("get_evidence_analysis", { jobId }),
  cancelEvidenceAnalysis: (jobId: string) =>
    invokeCommand<EvidenceAnalysisJobDto>("cancel_evidence_analysis", { jobId }),
  createBackup: () => invokeCommand<BackupResult>("create_backup"),
  exportJson: () => invokeCommand<ExportResult>("export_json"),
  getReleaseInfo: () => invokeCommand<ReleaseInfoDto>("get_release_info"),
  checkForAppUpdate: () => invokeCommand<AppUpdateDto | null>("check_for_app_update"),
  installAppUpdate: (onEvent: (event: AppUpdateEvent) => void) => {
    const channel = new Channel<AppUpdateEvent>();
    channel.onmessage = onEvent;
    return invokeCommand<void>("install_app_update", { onEvent: channel });
  },
};
