export type JobStatus = "pending" | "running" | "succeeded" | "failed" | "cancelled" | "confirmed";
export type CandidateDecision = "pending" | "accepted" | "rejected" | "edited";
export type QuestStatus =
  | "proposed"
  | "accepted"
  | "in_progress"
  | "completed"
  | "rescheduled"
  | "adjusted"
  | "cancelled";

export interface BootState {
  onboardingComplete: boolean;
  codex: CodexConnectionStatus | null;
}

export interface OnboardingInput {
  role: string;
  focusSkillIds: string[];
  weeklyMinutes: number;
  excludedQuestPatterns: string;
  codexPath: string;
}

export interface AppSettingsDto extends OnboardingInput {
  onboardingComplete: boolean;
  updatedAt: string;
}

export interface CodexConnectionStatus {
  available: boolean;
  authenticated: boolean;
  path: string;
  version: string | null;
  message: string;
}

export interface CreateActivityInput {
  occurredOn: string;
  actionText: string;
  challengeText: string;
  outcomeText: string;
}

export interface ActivityDto extends CreateActivityInput {
  id: string;
  createdAt: string;
  analysisStatus: JobStatus | null;
}

export interface ActivityDetailDto extends ActivityDto {
  analyses: ActivityAnalysisDto[];
}

export interface AnalysisPreview {
  activityId: string;
  submittedPayload: string;
  cloudInferenceNotice: string;
}

export interface StartAnalysisInput {
  activityId: string;
  submittedPayload: string;
}

export interface AnalysisJobDto {
  id: string;
  activityId: string;
  status: JobStatus;
  errorMessage: string | null;
  createdAt: string;
  completedAt: string | null;
}

export interface SkillCandidateDto {
  id: string;
  skillId: string;
  confidence: number;
  reason: string;
  evidence: string;
  decision: CandidateDecision;
}

export interface ActivityAnalysisDto {
  id: string;
  activityId: string;
  status: JobStatus;
  summary: string | null;
  outcomes: string[];
  skillCandidates: SkillCandidateDto[];
  missingInformationQuestion: string | null;
  errorMessage: string | null;
}

export interface CandidateDecisionInput {
  candidateId: string;
  decision: Exclude<CandidateDecision, "pending">;
  editedReason: string | null;
  editedEvidence: string | null;
}

export interface ConfirmAnalysisInput {
  analysisId: string;
  candidateDecisions: CandidateDecisionInput[];
}

export interface ConfirmAnalysisResult {
  analysisId: string;
  confirmedObservationCount: number;
  xpAwarded: number;
}

export interface QuestDto {
  id: string;
  templateId: string;
  title: string;
  description: string;
  targetSkillId: string;
  estimatedMinutes: number;
  difficulty: number;
  successCriteria: string[];
  evidencePrompt: string;
  status: QuestStatus;
  scheduledOn: string | null;
}

export interface GenerateQuestInput {
  activityId: string;
  analysisId: string;
}

export type QuestTransitionAction =
  | "accept"
  | "start"
  | "complete"
  | "reschedule"
  | "adjust"
  | "cancel";

export interface QuestTransitionInput {
  questId: string;
  action: QuestTransitionAction;
  scheduledOn: string | null;
  estimatedMinutes: number | null;
}

export type QuestReflectionResult = "completed" | "partially_completed" | "not_completed" | "rested";

export interface QuestReflectionInput {
  questId: string;
  result: QuestReflectionResult;
  learned: string;
  difficultyActual: number | null;
  nextAction: string;
}

export interface QuestReflectionDto extends QuestReflectionInput {
  id: string;
  createdAt: string;
  xpAwarded: number;
}

export interface SkillDto {
  id: string;
  code: string;
  name: string;
  category: string;
  evidenceCount: number;
  state: "observing";
}

export interface DashboardSnapshot {
  level: number;
  totalXp: number;
  xpToNextLevel: number;
  todayXp: number;
  todayActivities: number;
  todayObservations: number;
  activeQuest: QuestDto | null;
  recentActivities: ActivityDto[];
  weeklyXp: Array<{ date: string; xp: number }>;
  categoryObservations: Array<{ category: string; count: number }>;
}

export interface BackupResult {
  path: string;
  createdAt: string;
}

export interface ExportResult {
  path: string;
  schemaVersion: number;
  exportedAt: string;
}
