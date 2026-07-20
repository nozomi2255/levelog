CREATE TABLE source_documents (
    id TEXT PRIMARY KEY NOT NULL,
    content_sha256 TEXT NOT NULL UNIQUE,
    content_text TEXT NOT NULL,
    byte_length INTEGER NOT NULL CHECK (byte_length >= 0),
    line_count INTEGER NOT NULL CHECK (line_count >= 0),
    created_at TEXT NOT NULL
);

CREATE TABLE source_occurrences (
    id TEXT PRIMARY KEY NOT NULL,
    source_document_id TEXT NOT NULL REFERENCES source_documents(id) ON DELETE RESTRICT,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('paste', 'markdown', 'text')),
    display_name TEXT NOT NULL,
    original_path TEXT,
    imported_at TEXT NOT NULL
);
CREATE INDEX idx_source_occurrences_document ON source_occurrences(source_document_id, imported_at DESC);

CREATE TABLE evidence_claims (
    id TEXT PRIMARY KEY NOT NULL,
    source_document_id TEXT NOT NULL REFERENCES source_documents(id) ON DELETE RESTRICT,
    source_occurrence_id TEXT REFERENCES source_occurrences(id) ON DELETE RESTRICT,
    supersedes_claim_id TEXT REFERENCES evidence_claims(id) ON DELETE RESTRICT,
    kind TEXT NOT NULL CHECK (kind IN ('fact', 'experience', 'achievement', 'outcome', 'decision', 'lesson', 'knowledge', 'idea', 'project', 'interest', 'personality_signal', 'inference')),
    provenance TEXT NOT NULL CHECK (provenance IN ('user_asserted', 'import_extracted', 'ai_inference', 'activity_confirmed')),
    statement TEXT NOT NULL,
    source_excerpt TEXT NOT NULL,
    start_byte INTEGER,
    end_byte INTEGER,
    confidence REAL CHECK (confidence IS NULL OR (confidence >= 0 AND confidence <= 1)),
    review_state TEXT NOT NULL DEFAULT 'pending' CHECK (review_state IN ('pending', 'accepted', 'edited', 'rejected', 'excluded', 'deferred')),
    portfolio_eligible INTEGER NOT NULL DEFAULT 0 CHECK (portfolio_eligible IN (0, 1)),
    created_at TEXT NOT NULL,
    reviewed_at TEXT,
    CHECK ((start_byte IS NULL AND end_byte IS NULL) OR (start_byte IS NOT NULL AND end_byte IS NOT NULL AND start_byte >= 0 AND end_byte >= start_byte))
);
CREATE INDEX idx_evidence_claims_review ON evidence_claims(review_state, created_at DESC);
CREATE INDEX idx_evidence_claims_source ON evidence_claims(source_document_id, created_at DESC);

CREATE TABLE evidence_relations (
    id TEXT PRIMARY KEY NOT NULL,
    from_claim_id TEXT NOT NULL REFERENCES evidence_claims(id) ON DELETE RESTRICT,
    to_claim_id TEXT NOT NULL REFERENCES evidence_claims(id) ON DELETE RESTRICT,
    relation_type TEXT NOT NULL CHECK (relation_type IN ('supports', 'contradicts', 'refines', 'duplicates', 'derived_from', 'related')),
    created_by TEXT NOT NULL CHECK (created_by IN ('user', 'import', 'ai_suggestion')),
    created_at TEXT NOT NULL,
    UNIQUE(from_claim_id, to_claim_id, relation_type),
    CHECK (from_claim_id != to_claim_id)
);

CREATE TABLE evidence_claim_activity_links (
    claim_id TEXT NOT NULL REFERENCES evidence_claims(id) ON DELETE RESTRICT,
    activity_id TEXT NOT NULL REFERENCES activities(id) ON DELETE RESTRICT,
    created_at TEXT NOT NULL,
    PRIMARY KEY (claim_id, activity_id)
);

CREATE TABLE evidence_claim_skill_links (
    claim_id TEXT NOT NULL REFERENCES evidence_claims(id) ON DELETE RESTRICT,
    skill_id TEXT NOT NULL REFERENCES skills(id) ON DELETE RESTRICT,
    created_at TEXT NOT NULL,
    PRIMARY KEY (claim_id, skill_id)
);

CREATE TABLE projects (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('idea', 'active', 'paused', 'completed', 'archived')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE project_evidence_links (
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    claim_id TEXT NOT NULL REFERENCES evidence_claims(id) ON DELETE RESTRICT,
    created_at TEXT NOT NULL,
    PRIMARY KEY (project_id, claim_id)
);

CREATE TABLE portfolio_drafts (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    purpose TEXT NOT NULL,
    body_markdown TEXT NOT NULL,
    privacy_state TEXT NOT NULL DEFAULT 'private' CHECK (privacy_state = 'private'),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE portfolio_draft_items (
    draft_id TEXT NOT NULL REFERENCES portfolio_drafts(id) ON DELETE CASCADE,
    claim_id TEXT NOT NULL REFERENCES evidence_claims(id) ON DELETE RESTRICT,
    sort_order INTEGER NOT NULL CHECK (sort_order >= 0),
    created_at TEXT NOT NULL,
    PRIMARY KEY (draft_id, claim_id),
    UNIQUE(draft_id, sort_order)
);

CREATE TABLE evidence_analysis_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    source_document_id TEXT NOT NULL REFERENCES source_documents(id) ON DELETE RESTRICT,
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
    completed_at TEXT
);
CREATE INDEX idx_evidence_analysis_source ON evidence_analysis_jobs(source_document_id, created_at DESC);
CREATE UNIQUE INDEX idx_one_active_evidence_analysis_per_source
ON evidence_analysis_jobs(source_document_id)
WHERE status IN ('pending', 'running');
