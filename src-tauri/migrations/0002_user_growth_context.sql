PRAGMA foreign_keys = ON;

CREATE TABLE user_profile_revisions (
  id TEXT PRIMARY KEY NOT NULL,
  schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
  revision INTEGER NOT NULL UNIQUE CHECK (revision >= 1),
  profile_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  supersedes_id TEXT REFERENCES user_profile_revisions(id)
);

CREATE TABLE focus_themes (
  id TEXT PRIMARY KEY NOT NULL,
  title TEXT NOT NULL,
  desired_outcome TEXT NOT NULL DEFAULT '',
  why_now TEXT NOT NULL DEFAULT '',
  horizon TEXT NOT NULL DEFAULT 'ongoing' CHECK (horizon IN ('now', 'quarter', 'year', 'ongoing')),
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'paused', 'completed')),
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE focus_theme_skill_links (
  theme_id TEXT NOT NULL REFERENCES focus_themes(id) ON DELETE CASCADE,
  skill_id TEXT NOT NULL REFERENCES skills(id),
  relevance REAL NOT NULL DEFAULT 1 CHECK (relevance >= 0 AND relevance <= 1),
  created_at TEXT NOT NULL,
  PRIMARY KEY (theme_id, skill_id)
);

CREATE TABLE activity_captures (
  id TEXT PRIMARY KEY NOT NULL,
  activity_id TEXT NOT NULL UNIQUE REFERENCES activities(id),
  raw_text TEXT NOT NULL,
  capture_mode TEXT NOT NULL DEFAULT 'quick' CHECK (capture_mode IN ('quick', 'guided', 'deep')),
  created_at TEXT NOT NULL
);

CREATE TABLE activity_workflows (
  activity_id TEXT PRIMARY KEY NOT NULL REFERENCES activities(id),
  state TEXT NOT NULL CHECK (state IN ('captured', 'analysis_running', 'needs_input', 'assessable', 'review_pending', 'confirmed', 'excluded')),
  version INTEGER NOT NULL DEFAULT 1 CHECK (version >= 1),
  updated_at TEXT NOT NULL
);

CREATE TABLE activity_structures (
  id TEXT PRIMARY KEY NOT NULL,
  activity_id TEXT NOT NULL REFERENCES activities(id),
  analysis_id TEXT REFERENCES ai_analyses(id),
  revision INTEGER NOT NULL CHECK (revision >= 1),
  structured_json TEXT NOT NULL,
  source TEXT NOT NULL DEFAULT 'codex_analysis',
  prompt_version TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE (activity_id, revision)
);

CREATE TABLE interview_sessions (
  id TEXT PRIMARY KEY NOT NULL,
  activity_id TEXT NOT NULL REFERENCES activities(id),
  analysis_id TEXT NOT NULL UNIQUE REFERENCES ai_analyses(id),
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'answered', 'unknown', 'skipped', 'deferred', 'closed')),
  current_question_json TEXT NOT NULL,
  prompt_version TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE interview_answers (
  id TEXT PRIMARY KEY NOT NULL,
  session_id TEXT NOT NULL REFERENCES interview_sessions(id),
  question_id TEXT NOT NULL,
  answer_json TEXT,
  answer_state TEXT NOT NULL CHECK (answer_state IN ('answered', 'unknown', 'skipped', 'deferred')),
  created_at TEXT NOT NULL
);

CREATE TABLE quest_generation_runs (
  id TEXT PRIMARY KEY NOT NULL,
  activity_id TEXT NOT NULL REFERENCES activities(id),
  analysis_id TEXT NOT NULL REFERENCES ai_analyses(id),
  quest_id TEXT REFERENCES quests(id),
  status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'succeeded', 'failed', 'cancelled')),
  submitted_payload TEXT NOT NULL,
  raw_result_json TEXT,
  provider TEXT NOT NULL DEFAULT 'codex-cli',
  prompt_version TEXT NOT NULL,
  schema_version TEXT NOT NULL,
  error_message TEXT,
  created_at TEXT NOT NULL,
  completed_at TEXT
);

ALTER TABLE skill_candidates ADD COLUMN specialized_skill_name TEXT;
ALTER TABLE skill_candidates ADD COLUMN normalized_specialized_skill_name TEXT;
ALTER TABLE skill_observations ADD COLUMN specialized_skill_name TEXT;
ALTER TABLE skill_observations ADD COLUMN normalized_specialized_skill_name TEXT;
ALTER TABLE quests ADD COLUMN focus_theme_id TEXT REFERENCES focus_themes(id);

INSERT INTO activity_workflows (activity_id, state, version, updated_at)
SELECT
  a.id,
  CASE
    WHEN EXISTS (SELECT 1 FROM ai_analyses x WHERE x.activity_id = a.id AND x.status = 'confirmed') THEN 'confirmed'
    WHEN EXISTS (SELECT 1 FROM ai_analyses x WHERE x.activity_id = a.id AND x.status IN ('pending', 'running')) THEN 'analysis_running'
    WHEN EXISTS (SELECT 1 FROM ai_analyses x WHERE x.activity_id = a.id AND x.status = 'succeeded') THEN 'review_pending'
    ELSE 'assessable'
  END,
  1,
  a.created_at
FROM activities a;

CREATE INDEX idx_user_profile_revisions_revision ON user_profile_revisions(revision DESC);
CREATE INDEX idx_focus_themes_status_order ON focus_themes(status, sort_order);
CREATE INDEX idx_activity_workflows_state ON activity_workflows(state, updated_at DESC);
CREATE INDEX idx_activity_structures_activity_revision ON activity_structures(activity_id, revision DESC);
CREATE INDEX idx_interview_sessions_activity_status ON interview_sessions(activity_id, status, updated_at DESC);
CREATE UNIQUE INDEX idx_interview_sessions_one_open_per_activity
  ON interview_sessions(activity_id)
  WHERE status IN ('pending', 'deferred');
CREATE INDEX idx_interview_answers_session ON interview_answers(session_id, created_at);
CREATE INDEX idx_interview_answers_question ON interview_answers(session_id, question_id, created_at DESC);
CREATE INDEX idx_quest_generation_runs_analysis_created ON quest_generation_runs(analysis_id, created_at DESC);
CREATE INDEX idx_skill_observations_specialized ON skill_observations(normalized_specialized_skill_name);

-- Older builds did not enforce one open job per activity. Preserve every row while
-- closing all but the newest open job before adding the invariant.
UPDATE ai_analyses
SET status = 'failed',
    error_message = COALESCE(error_message, '新しい解析に置き換えられました'),
    completed_at = COALESCE(completed_at, datetime('now'))
WHERE status IN ('pending', 'running')
  AND EXISTS (
    SELECT 1
    FROM ai_analyses newer
    WHERE newer.activity_id = ai_analyses.activity_id
      AND newer.status IN ('pending', 'running')
      AND (
        newer.created_at > ai_analyses.created_at
        OR (newer.created_at = ai_analyses.created_at AND newer.rowid > ai_analyses.rowid)
      )
  );

CREATE UNIQUE INDEX idx_ai_analyses_one_running_per_activity
  ON ai_analyses(activity_id)
  WHERE status IN ('pending', 'running');
