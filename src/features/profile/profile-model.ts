import type { UpdateUserProfileInput, UserProfileDto } from "../../lib/types";

export type ProfileFormValue = UpdateUserProfileInput;

export const emptyProfile: ProfileFormValue = {
  expectedRevision: null, role: "", background: "", currentResponsibilities: "", domainsAndTechnologies: [], growthGoal: "", motivation: "", currentChallenges: "", recentSuccess: "", focusSkillIds: [], weeklyMinutes: 120, preferredQuestMinutes: 15, preferredQuestStyle: "balanced", constraints: "", excludedQuestPatterns: "",
};

export function profileToInput(profile: UserProfileDto): ProfileFormValue {
  return { expectedRevision: profile.revision, role: profile.role, background: profile.background, currentResponsibilities: profile.currentResponsibilities, domainsAndTechnologies: profile.domainsAndTechnologies, growthGoal: profile.growthGoal, motivation: profile.motivation, currentChallenges: profile.currentChallenges, recentSuccess: profile.recentSuccess, focusSkillIds: profile.focusSkillIds, weeklyMinutes: profile.weeklyMinutes, preferredQuestMinutes: profile.preferredQuestMinutes, preferredQuestStyle: profile.preferredQuestStyle, constraints: profile.constraints, excludedQuestPatterns: profile.excludedQuestPatterns };
}
