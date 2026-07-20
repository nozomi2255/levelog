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
  FocusThemeDto,
  FocusThemeInput,
  GenerateQuestInput,
  OnboardingInput,
  InterviewAnswerInput,
  QuickCaptureInput,
  QuestDto,
  QuestReflectionDto,
  QuestReflectionInput,
  QuestTransitionInput,
  SkillDto,
  StartAnalysisInput,
  SubmissionPreview,
  UpdateUserProfileInput,
  UserProfileDto,
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
  createBackup: () => invokeCommand<BackupResult>("create_backup"),
  exportJson: () => invokeCommand<ExportResult>("export_json"),
};
