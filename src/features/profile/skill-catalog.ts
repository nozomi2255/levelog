export type SkillGroup = readonly [id: string, label: string, skills: ReadonlyArray<readonly [id: string, label: string]>];

export const SKILL_GROUPS: ReadonlyArray<SkillGroup> = [
  ["thinking", "思考力", [["thinking.information_structuring", "情報整理"], ["thinking.problem_decomposition", "問題分解"], ["thinking.hypothesis_testing", "仮説検証"]]],
  ["technical", "技術力", [["technical.technical_learning", "技術学習"], ["technical.system_design", "システム設計"], ["technical.validation", "検証"]]],
  ["communication", "伝える力", [["communication.clarification", "確認質問"], ["communication.explanation", "説明"], ["communication.documentation", "文章化"]]],
  ["execution", "実行力", [["execution.prioritization", "優先順位"], ["execution.planning", "計画"], ["execution.follow_through", "やり切る"]]],
  ["interpersonal", "対人力", [["interpersonal.listening", "傾聴"], ["interpersonal.alignment", "認識合わせ"], ["interpersonal.feedback", "フィードバック"]]],
] as const;

export const SKILL_NAMES = new Map<string, string>(SKILL_GROUPS.flatMap(([, , skills]) => [...skills]));
export const SKILL_ORDER: string[] = SKILL_GROUPS.flatMap(([, , skills]) => skills.map(([id]) => id));
export const CATEGORY_NAMES: Record<string, string> = Object.fromEntries(SKILL_GROUPS.map(([id, name]) => [id, name]));
