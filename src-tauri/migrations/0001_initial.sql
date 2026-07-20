PRAGMA foreign_keys = ON;

CREATE TABLE activities (
  id TEXT PRIMARY KEY NOT NULL,
  occurred_on TEXT NOT NULL,
  action_text TEXT NOT NULL DEFAULT '',
  challenge_text TEXT NOT NULL DEFAULT '',
  outcome_text TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL
);

CREATE TABLE skills (
  id TEXT PRIMARY KEY NOT NULL,
  category TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT NOT NULL,
  is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1))
);

CREATE TABLE ai_analyses (
  id TEXT PRIMARY KEY NOT NULL,
  activity_id TEXT NOT NULL REFERENCES activities(id),
  status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'succeeded', 'failed', 'cancelled', 'confirmed')),
  submitted_payload TEXT NOT NULL,
  raw_result_json TEXT,
  provider TEXT NOT NULL,
  model TEXT,
  codex_version TEXT,
  prompt_version TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  error_message TEXT,
  created_at TEXT NOT NULL,
  completed_at TEXT,
  confirmed_at TEXT
);

CREATE TABLE skill_candidates (
  id TEXT PRIMARY KEY NOT NULL,
  analysis_id TEXT NOT NULL REFERENCES ai_analyses(id),
  skill_id TEXT NOT NULL REFERENCES skills(id),
  confidence REAL NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
  reason TEXT NOT NULL,
  evidence TEXT NOT NULL,
  decision TEXT NOT NULL DEFAULT 'pending' CHECK (decision IN ('pending', 'accepted', 'rejected', 'edited')),
  edited_reason TEXT,
  edited_evidence TEXT,
  decided_at TEXT,
  UNIQUE (analysis_id, skill_id)
);

CREATE TABLE skill_observations (
  id TEXT PRIMARY KEY NOT NULL,
  activity_id TEXT NOT NULL REFERENCES activities(id),
  analysis_id TEXT NOT NULL REFERENCES ai_analyses(id),
  skill_id TEXT NOT NULL REFERENCES skills(id),
  evidence TEXT NOT NULL,
  source TEXT NOT NULL DEFAULT 'analysis_confirmation',
  created_at TEXT NOT NULL,
  UNIQUE (analysis_id, skill_id)
);

CREATE TABLE quests (
  id TEXT PRIMARY KEY NOT NULL,
  template_id TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT NOT NULL,
  quest_type TEXT NOT NULL DEFAULT 'daily',
  status TEXT NOT NULL CHECK (status IN ('proposed', 'accepted', 'in_progress', 'completed', 'rescheduled', 'adjusted', 'cancelled')),
  target_skill_id TEXT REFERENCES skills(id),
  difficulty INTEGER NOT NULL CHECK (difficulty BETWEEN 1 AND 5),
  estimated_minutes INTEGER NOT NULL CHECK (estimated_minutes > 0),
  success_criteria_json TEXT NOT NULL,
  evidence_prompt TEXT NOT NULL,
  scheduled_on TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE quest_reflections (
  id TEXT PRIMARY KEY NOT NULL,
  quest_id TEXT NOT NULL UNIQUE REFERENCES quests(id),
  result TEXT NOT NULL CHECK (result IN ('completed', 'partially_completed', 'not_completed', 'rested')),
  learned TEXT NOT NULL DEFAULT '',
  difficulty_actual INTEGER CHECK (difficulty_actual IS NULL OR difficulty_actual BETWEEN 1 AND 5),
  next_action TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL
);

CREATE TABLE xp_events (
  id TEXT PRIMARY KEY NOT NULL,
  amount INTEGER NOT NULL CHECK (amount > 0),
  reason_type TEXT NOT NULL CHECK (reason_type IN ('activity_saved', 'analysis_confirmed', 'quest_reflection_saved')),
  reason_key TEXT NOT NULL UNIQUE,
  activity_id TEXT REFERENCES activities(id),
  analysis_id TEXT REFERENCES ai_analyses(id),
  quest_id TEXT REFERENCES quests(id),
  description TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE app_settings (
  key TEXT PRIMARY KEY NOT NULL,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_activities_occurred_on ON activities(occurred_on);
CREATE INDEX idx_ai_analyses_activity_id ON ai_analyses(activity_id);
CREATE INDEX idx_skill_candidates_analysis_id ON skill_candidates(analysis_id);
CREATE INDEX idx_skill_observations_skill_id ON skill_observations(skill_id);
CREATE INDEX idx_quests_status_scheduled_on ON quests(status, scheduled_on);
CREATE INDEX idx_xp_events_created_at ON xp_events(created_at);

INSERT INTO skills (id, category, name, description) VALUES
('thinking.information_structuring', 'thinking', '情報整理', '情報を構造化して要点を見出す'),
('thinking.problem_decomposition', 'thinking', '問題分解', '複雑な問題を扱える単位に分ける'),
('thinking.hypothesis_testing', 'thinking', '仮説検証', '仮説を立てて根拠で確かめる'),
('technical.technical_learning', 'technical', '技術学習', '新しい技術を理解し試す'),
('technical.system_design', 'technical', 'システム設計', '要件から適切な構造を設計する'),
('technical.validation', 'technical', '検証', '比較・テストで判断を確かめる'),
('communication.clarification', 'communication', '確認質問', '曖昧さを質問で明確にする'),
('communication.explanation', 'communication', '説明', '相手に合わせて分かりやすく伝える'),
('communication.documentation', 'communication', '文章化', '再利用できる形で知識を残す'),
('execution.prioritization', 'execution', '優先順位', '重要度に応じて順序を決める'),
('execution.planning', 'execution', '計画', '実行可能な段取りを組む'),
('execution.follow_through', 'execution', 'やり切る', '行動を最後まで進める'),
('interpersonal.listening', 'interpersonal', '傾聴', '相手の意図や前提を理解する'),
('interpersonal.alignment', 'interpersonal', '認識合わせ', '関係者の認識を揃える'),
('interpersonal.feedback', 'interpersonal', 'フィードバック', '改善につながる返答を行う');
