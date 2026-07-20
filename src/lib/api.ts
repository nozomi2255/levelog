import type {
  ActivityAnalysisDto,
  ActivityDetailDto,
  ActivityDto,
  AnalysisJobDto,
  AnalysisPreview,
  AppSettingsDto,
  BackupResult,
  BootState,
  CodexConnectionStatus,
  ConfirmAnalysisInput,
  ConfirmAnalysisResult,
  CreateActivityInput,
  DashboardSnapshot,
  ExportResult,
  GenerateQuestInput,
  OnboardingInput,
  QuestDto,
  QuestReflectionDto,
  QuestReflectionInput,
  QuestTransitionInput,
  SkillDto,
  StartAnalysisInput,
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
  getDashboard: () => invokeCommand<DashboardSnapshot>("get_dashboard"),
  createActivity: (input: CreateActivityInput) =>
    invokeCommand<ActivityDto>("create_activity", { input }),
  listActivities: () => invokeCommand<ActivityDto[]>("list_activities"),
  getActivity: (activityId: string) =>
    invokeCommand<ActivityDetailDto>("get_activity", { activityId }),
  getAnalysisPreview: (activityId: string) =>
    invokeCommand<AnalysisPreview>("get_analysis_preview", { activityId }),
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
  listQuests: () => invokeCommand<QuestDto[]>("list_quests"),
  transitionQuest: (input: QuestTransitionInput) =>
    invokeCommand<QuestDto>("transition_quest", { input }),
  saveQuestReflection: (input: QuestReflectionInput) =>
    invokeCommand<QuestReflectionDto>("save_quest_reflection", { input }),
  listSkills: () => invokeCommand<SkillDto[]>("list_skills"),
  createBackup: () => invokeCommand<BackupResult>("create_backup"),
  exportJson: () => invokeCommand<ExportResult>("export_json"),
};
