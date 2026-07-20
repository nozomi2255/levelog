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

export interface CodexPathCandidateDto {
  discoveredPath: string;
  canonicalPath: string;
  source: string;
  executable: boolean;
  recommended: boolean;
  connection: CodexConnectionStatus | null;
}

export type QuestStyle = "balanced" | "work_integrated" | "practice" | "reflection";

export interface FocusThemeDto {
  id: string;
  title: string;
  desiredOutcome: string;
  whyNow: string;
  horizon: "now" | "quarter" | "year" | "ongoing";
  status: "active" | "paused" | "completed";
  linkedSkillIds: string[];
  sortOrder: number;
  updatedAt: string;
}

export interface FocusThemeInput extends Omit<FocusThemeDto, "id" | "updatedAt"> {
  id: string | null;
}

export interface UserProfileDto {
  schemaVersion: number;
  revision: number;
  role: string;
  background: string;
  currentResponsibilities: string;
  domainsAndTechnologies: string[];
  growthGoal: string;
  motivation: string;
  currentChallenges: string;
  recentSuccess: string;
  focusSkillIds: string[];
  weeklyMinutes: number;
  preferredQuestMinutes: number;
  preferredQuestStyle: QuestStyle;
  constraints: string;
  excludedQuestPatterns: string;
  focusThemes: FocusThemeDto[];
  updatedAt: string;
}

export interface UpdateUserProfileInput {
  expectedRevision: number | null;
  role: string;
  background: string;
  currentResponsibilities: string;
  domainsAndTechnologies: string[];
  growthGoal: string;
  motivation: string;
  currentChallenges: string;
  recentSuccess: string;
  focusSkillIds: string[];
  weeklyMinutes: number;
  preferredQuestMinutes: number;
  preferredQuestStyle: QuestStyle;
  constraints: string;
  excludedQuestPatterns: string;
}

export interface CreateActivityInput {
  occurredOn: string;
  actionText: string;
  challengeText: string;
  outcomeText: string;
}

export type CaptureMode = "quick" | "guided" | "deep";

export interface QuickCaptureInput {
  occurredOn: string;
  rawText: string;
  captureMode: CaptureMode;
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
  specializedSkillName: string | null;
}

export interface InterviewChoiceDto {
  value: string;
  label: string;
}

export interface InterviewQuestionDto {
  sessionId: string;
  questionId: string;
  target: string;
  text: string;
  answerType: "single_choice" | "text" | "number";
  choices: InterviewChoiceDto[];
  whyItMatters: string;
  status: "pending" | "answered" | "unknown" | "skipped" | "deferred" | "closed";
}

export interface InterviewAnswerInput {
  sessionId: string;
  questionId: string;
  answerState: "answered" | "unknown" | "skipped" | "deferred";
  answer: string | null;
}

export interface ActivityWorkflowDto {
  activityId: string;
  state: "captured" | "analysis_running" | "needs_input" | "assessable" | "review_pending" | "confirmed" | "excluded";
  version: number;
  currentQuestion: InterviewQuestionDto | null;
  updatedAt: string;
}

export interface ActivityInboxItemDto extends ActivityDto {
  workflow: ActivityWorkflowDto;
}

export interface ActivityAnalysisDto {
  id: string;
  activityId: string;
  status: JobStatus;
  summary: string | null;
  outcomes: string[];
  confirmedFacts: string[];
  unconfirmedFacts: string[];
  skillCandidates: SkillCandidateDto[];
  missingInformationQuestion: string | null;
  nextQuestion: InterviewQuestionDto | null;
  errorMessage: string | null;
}

export interface CandidateDecisionInput {
  candidateId: string;
  decision: Exclude<CandidateDecision, "pending">;
  editedReason: string | null;
  editedEvidence: string | null;
  editedSkillId: string | null;
  editedSpecializedSkillName: string | null;
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
  submittedPayload?: string | null;
}

export interface SubmissionPreview {
  entityId: string;
  submittedPayload: string;
  cloudInferenceNotice: string;
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
  specializedSkills: Array<{
    name: string;
    evidenceCount: number;
    lastObservedAt: string | null;
  }>;
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

export type EvidenceClaimKind =
  | "fact"
  | "experience"
  | "achievement"
  | "outcome"
  | "decision"
  | "lesson"
  | "knowledge"
  | "idea"
  | "project"
  | "interest"
  | "personality_signal"
  | "inference";

export type EvidenceProvenance =
  | "user_asserted"
  | "import_extracted"
  | "ai_inference"
  | "activity_confirmed";

export type EvidenceReviewState =
  | "pending"
  | "accepted"
  | "edited"
  | "rejected"
  | "excluded"
  | "deferred";

export interface ImportPastedSourceInput {
  displayName: string;
  contentText: string;
}

export interface SourceDocumentDto {
  id: string;
  contentSha256: string;
  contentText: string;
  byteLength: number;
  lineCount: number;
  createdAt: string;
}

export interface SourceOccurrenceDto {
  id: string;
  sourceDocumentId: string;
  sourceKind: "paste" | "markdown" | "text";
  displayName: string;
  originalPath: string | null;
  importedAt: string;
}

export interface ImportedSourceDto {
  document: SourceDocumentDto;
  occurrence: SourceOccurrenceDto;
  duplicateContent: boolean;
}

export interface SourceImportResult {
  imported: ImportedSourceDto[];
  failures: Array<{ displayName: string; message: string }>;
}

export interface EvidenceClaimDto {
  id: string;
  sourceDocumentId: string;
  sourceOccurrenceId: string | null;
  supersedesClaimId: string | null;
  kind: EvidenceClaimKind;
  provenance: EvidenceProvenance;
  statement: string;
  sourceExcerpt: string;
  startByte: number | null;
  endByte: number | null;
  confidence: number | null;
  reviewState: EvidenceReviewState;
  portfolioEligible: boolean;
  linkedSkillIds: string[];
  createdAt: string;
  reviewedAt: string | null;
}

export interface EvidenceRelationDto {
  id: string;
  fromClaimId: string;
  toClaimId: string;
  relationType: "supports" | "contradicts" | "refines" | "duplicates" | "related";
  createdBy: "user" | "import" | "ai_suggestion";
  createdAt: string;
}

export interface CreateEvidenceRelationInput {
  fromClaimId: string;
  toClaimId: string;
  relationType: EvidenceRelationDto["relationType"];
}

export interface CreateEvidenceClaimInput {
  sourceDocumentId: string;
  sourceOccurrenceId: string | null;
  kind: EvidenceClaimKind;
  statement: string;
  sourceExcerpt: string;
  startByte: number | null;
  endByte: number | null;
  linkedSkillIds: string[];
}

export interface ReviewEvidenceClaimInput {
  claimId: string;
  decision: "accept" | "edit" | "reject" | "exclude" | "defer" | "reopen";
  editedStatement: string | null;
  portfolioEligible: boolean;
}

export interface ClaimActivityLinkInput {
  claimId: string;
  activityId: string;
}

export interface EvidenceLibraryQuery {
  reviewState: EvidenceReviewState | null;
  kind: EvidenceClaimKind | null;
  projectId: string | null;
  search: string | null;
}

export interface EvidenceLibraryDto {
  sources: SourceOccurrenceDto[];
  claims: EvidenceClaimDto[];
  counts: {
    sourceCount: number;
    pendingClaimCount: number;
    acceptedClaimCount: number;
    inferenceCount: number;
    projectCount: number;
    privateDraftCount: number;
  };
}

export interface SourceDocumentDetailDto {
  document: SourceDocumentDto;
  occurrences: SourceOccurrenceDto[];
  claims: EvidenceClaimDto[];
}

export type ProjectStatus = "idea" | "active" | "paused" | "completed" | "archived";

export interface CreateProjectInput {
  name: string;
  summary: string;
  status: ProjectStatus;
}

export interface ProjectDto extends CreateProjectInput {
  id: string;
  evidenceCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface ProjectDetailDto extends ProjectDto {
  claims: EvidenceClaimDto[];
}

export interface PortfolioDraftDto {
  id: string;
  title: string;
  purpose: string;
  bodyMarkdown: string;
  privacyState: "private";
  claimIds: string[];
  createdAt: string;
  updatedAt: string;
}

export interface CreatePortfolioDraftInput {
  title: string;
  purpose: string;
  claimIds: string[];
}

export interface UpdatePortfolioDraftInput extends CreatePortfolioDraftInput {
  draftId: string;
  bodyMarkdown: string;
}

export interface EvidenceAnalysisJobDto {
  id: string;
  sourceDocumentId: string;
  status: JobStatus;
  errorMessage: string | null;
  createdAt: string;
  completedAt: string | null;
}

export interface EvidenceAnalysisPreviewDto {
  sourceId: string;
  submittedPayload: string;
  cloudInferenceNotice: string;
  redactionFindings: Array<{
    kind: "private_key" | "password" | "api_key" | "token" | "email" | string;
    startByte: number;
    endByte: number;
  }>;
  needsReview: boolean;
}

export interface StartEvidenceAnalysisInput {
  sourceDocumentId: string;
  submittedPayload: string;
}

export interface ReleaseInfoDto {
  currentVersion: string;
  updaterConfigured: boolean;
  releaseChannel: string;
}

export interface AppUpdateDto {
  currentVersion: string;
  version: string;
  publishedAt: string | null;
  notes: string | null;
}

export type AppUpdateEvent =
  | { event: "started"; data: { contentLength: number | null } }
  | { event: "progress"; data: { chunkLength: number } }
  | { event: "finished" }
  | { event: "installed" };
