use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use std::collections::HashSet;
use uuid::Uuid;

use crate::{dto::*, error::AppError};

const KINDS: &[&str] = &[
    "fact",
    "experience",
    "achievement",
    "outcome",
    "decision",
    "lesson",
    "knowledge",
    "idea",
    "project",
    "interest",
    "personality_signal",
    "inference",
];
fn now() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
fn nonblank(v: &str, label: &str) -> Result<(), AppError> {
    if v.trim().is_empty() || v.chars().count() > 20_000 {
        Err(AppError::Validation(format!(
            "{label} must be non-empty and at most 20,000 characters"
        )))
    } else {
        Ok(())
    }
}
fn bounded(v: &str, label: &str, max_chars: usize) -> Result<(), AppError> {
    if v.trim().is_empty() || v.chars().count() > max_chars {
        Err(AppError::Validation(format!(
            "{label} must be non-empty and at most {max_chars} characters"
        )))
    } else {
        Ok(())
    }
}
fn validate_source(content: &str) -> Result<(), AppError> {
    if content.is_empty() {
        return Err(AppError::Validation(
            "source content must not be empty".into(),
        ));
    }
    if content.len() > 1024 * 1024 {
        return Err(AppError::Validation(
            "source content must be at most 1 MiB".into(),
        ));
    }
    if content.as_bytes().contains(&0) {
        return Err(AppError::Validation(
            "source content must not contain NUL".into(),
        ));
    }
    Ok(())
}
fn eligible_kind(k: &str) -> bool {
    !matches!(k, "inference" | "personality_signal" | "knowledge" | "idea")
}

pub async fn store_source(
    pool: &SqlitePool,
    source_kind: &str,
    display_name: &str,
    original_path: Option<&str>,
    content: &str,
) -> Result<ImportedSourceDto, AppError> {
    if !matches!(source_kind, "paste" | "markdown" | "text") {
        return Err(AppError::Validation("unsupported source kind".into()));
    }
    nonblank(display_name, "display name")?;
    validate_source(content)?;
    let hash = hex::encode(Sha256::digest(content.as_bytes()));
    let stamp = now();
    let mut tx = pool.begin().await?;
    let existing = sqlx::query("SELECT id, content_sha256, content_text, byte_length, line_count, created_at FROM source_documents WHERE content_sha256=?").bind(&hash).fetch_optional(&mut *tx).await?;
    let (doc, duplicate) = if let Some(r) = existing {
        let stored = source_from(&r);
        if stored.content_text != content {
            return Err(AppError::Conflict(
                "source hash matched different stored content".into(),
            ));
        }
        (stored, true)
    } else {
        let id = Uuid::new_v4().to_string();
        let lines = content.lines().count() as i64;
        sqlx::query("INSERT INTO source_documents (id,content_sha256,content_text,byte_length,line_count,created_at) VALUES (?,?,?,?,?,?)").bind(&id).bind(&hash).bind(content).bind(content.len() as i64).bind(lines).bind(&stamp).execute(&mut *tx).await?;
        (
            SourceDocumentDto {
                id,
                content_sha256: hash,
                content_text: content.into(),
                byte_length: content.len() as i64,
                line_count: lines,
                created_at: stamp.clone(),
            },
            false,
        )
    };
    let occurrence = SourceOccurrenceDto {
        id: Uuid::new_v4().to_string(),
        source_document_id: doc.id.clone(),
        source_kind: source_kind.into(),
        display_name: display_name.into(),
        original_path: original_path.map(str::to_owned),
        imported_at: stamp,
    };
    sqlx::query("INSERT INTO source_occurrences (id,source_document_id,source_kind,display_name,original_path,imported_at) VALUES (?,?,?,?,?,?)").bind(&occurrence.id).bind(&occurrence.source_document_id).bind(&occurrence.source_kind).bind(&occurrence.display_name).bind(&occurrence.original_path).bind(&occurrence.imported_at).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(ImportedSourceDto {
        document: doc,
        occurrence,
        duplicate_content: duplicate,
    })
}
fn source_from(r: &sqlx::sqlite::SqliteRow) -> SourceDocumentDto {
    SourceDocumentDto {
        id: r.get("id"),
        content_sha256: r.get("content_sha256"),
        content_text: r.get("content_text"),
        byte_length: r.get("byte_length"),
        line_count: r.get("line_count"),
        created_at: r.get("created_at"),
    }
}
fn occurrence_from(r: &sqlx::sqlite::SqliteRow) -> SourceOccurrenceDto {
    SourceOccurrenceDto {
        id: r.get("id"),
        source_document_id: r.get("source_document_id"),
        source_kind: r.get("source_kind"),
        display_name: r.get("display_name"),
        original_path: r.get("original_path"),
        imported_at: r.get("imported_at"),
    }
}

async fn claim_from(
    pool: &SqlitePool,
    r: &sqlx::sqlite::SqliteRow,
) -> Result<EvidenceClaimDto, AppError> {
    let id: String = r.get("id");
    let skills = sqlx::query_scalar(
        "SELECT skill_id FROM evidence_claim_skill_links WHERE claim_id=? ORDER BY skill_id",
    )
    .bind(&id)
    .fetch_all(pool)
    .await?;
    Ok(EvidenceClaimDto {
        id,
        source_document_id: r.get("source_document_id"),
        source_occurrence_id: r.get("source_occurrence_id"),
        supersedes_claim_id: r.get("supersedes_claim_id"),
        kind: r.get("kind"),
        provenance: r.get("provenance"),
        statement: r.get("statement"),
        source_excerpt: r.get("source_excerpt"),
        start_byte: r.get("start_byte"),
        end_byte: r.get("end_byte"),
        confidence: r.get("confidence"),
        review_state: r.get("review_state"),
        portfolio_eligible: r.get::<i64, _>("portfolio_eligible") != 0,
        linked_skill_ids: skills,
        created_at: r.get("created_at"),
        reviewed_at: r.get("reviewed_at"),
    })
}
fn relation_from(r: &sqlx::sqlite::SqliteRow) -> EvidenceRelationDto {
    EvidenceRelationDto {
        id: r.get("id"),
        from_claim_id: r.get("from_claim_id"),
        to_claim_id: r.get("to_claim_id"),
        relation_type: r.get("relation_type"),
        created_by: r.get("created_by"),
        created_at: r.get("created_at"),
    }
}

pub async fn list_relations(pool: &SqlitePool) -> Result<Vec<EvidenceRelationDto>, AppError> {
    Ok(
        sqlx::query("SELECT * FROM evidence_relations ORDER BY created_at DESC, rowid DESC")
            .fetch_all(pool)
            .await?
            .iter()
            .map(relation_from)
            .collect(),
    )
}

pub async fn create_relation(
    pool: &SqlitePool,
    input: CreateEvidenceRelationInput,
) -> Result<EvidenceRelationDto, AppError> {
    if !matches!(
        input.relation_type.as_str(),
        "supports" | "contradicts" | "refines" | "duplicates" | "related"
    ) {
        return Err(AppError::Validation("invalid user relation type".into()));
    }
    if input.from_claim_id == input.to_claim_id {
        return Err(AppError::Validation(
            "a claim cannot relate to itself".into(),
        ));
    }
    let mut tx = pool.begin().await?;
    for id in [&input.from_claim_id, &input.to_claim_id] {
        let state: Option<String> =
            sqlx::query_scalar("SELECT review_state FROM evidence_claims WHERE id=?")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        match state.as_deref() {
            Some("accepted") => {}
            Some(_) => {
                return Err(AppError::InvalidState(
                    "relations require accepted live claims".into(),
                ));
            }
            None => return Err(AppError::NotFound("claim".into())),
        }
    }
    if let Some(row) = sqlx::query("SELECT * FROM evidence_relations WHERE from_claim_id=? AND to_claim_id=? AND relation_type=?")
        .bind(&input.from_claim_id).bind(&input.to_claim_id).bind(&input.relation_type)
        .fetch_optional(&mut *tx).await? {
        let relation = relation_from(&row);
        tx.commit().await?;
        return Ok(relation);
    }
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO evidence_relations (id,from_claim_id,to_claim_id,relation_type,created_by,created_at) VALUES (?,?,?,?,'user',?)")
        .bind(&id).bind(&input.from_claim_id).bind(&input.to_claim_id).bind(&input.relation_type).bind(now())
        .execute(&mut *tx).await?;
    let row = sqlx::query("SELECT * FROM evidence_relations WHERE id=?")
        .bind(&id)
        .fetch_one(&mut *tx)
        .await?;
    let relation = relation_from(&row);
    tx.commit().await?;
    Ok(relation)
}

pub async fn delete_relation(pool: &SqlitePool, id: &str) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM evidence_relations WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() != 1 {
        return Err(AppError::NotFound("evidence relation".into()));
    }
    Ok(())
}
fn check_excerpt(
    content: &str,
    excerpt: &str,
    start: Option<i64>,
    end: Option<i64>,
) -> Result<(), AppError> {
    nonblank(excerpt, "source excerpt")?;
    match (start, end) {
        (None, None) => {
            if content.contains(excerpt) {
                Ok(())
            } else {
                Err(AppError::Validation(
                    "excerpt is not present in source".into(),
                ))
            }
        }
        (Some(s), Some(e)) => {
            let (s, e) = (s as usize, e as usize);
            if s > e
                || e > content.len()
                || !content.is_char_boundary(s)
                || !content.is_char_boundary(e)
                || &content[s..e] != excerpt
            {
                Err(AppError::Validation(
                    "byte anchor must be UTF-8 aligned and exactly match excerpt".into(),
                ))
            } else {
                Ok(())
            }
        }
        _ => Err(AppError::Validation(
            "both byte anchors are required".into(),
        )),
    }
}

pub async fn create_claim(
    pool: &SqlitePool,
    input: CreateEvidenceClaimInput,
) -> Result<EvidenceClaimDto, AppError> {
    if !KINDS.contains(&input.kind.as_str()) {
        return Err(AppError::Validation("invalid claim kind".into()));
    }
    nonblank(&input.statement, "statement")?;
    let mut tx = pool.begin().await?;
    let content: Option<String> =
        sqlx::query_scalar("SELECT content_text FROM source_documents WHERE id=?")
            .bind(&input.source_document_id)
            .fetch_optional(&mut *tx)
            .await?;
    let content = content.ok_or_else(|| AppError::NotFound("source document".into()))?;
    check_excerpt(
        &content,
        &input.source_excerpt,
        input.start_byte,
        input.end_byte,
    )?;
    if let Some(o) = &input.source_occurrence_id {
        let valid: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM source_occurrences WHERE id=? AND source_document_id=?",
        )
        .bind(o)
        .bind(&input.source_document_id)
        .fetch_one(&mut *tx)
        .await?;
        if valid != 1 {
            return Err(AppError::Validation(
                "occurrence does not belong to source".into(),
            ));
        }
    }
    for s in &input.linked_skill_ids {
        let ok: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM skills WHERE id=?")
            .bind(s)
            .fetch_one(&mut *tx)
            .await?;
        if ok != 1 {
            return Err(AppError::Validation(format!("unknown skill: {s}")));
        }
    }
    let id = Uuid::new_v4().to_string();
    let stamp = now();
    sqlx::query("INSERT INTO evidence_claims (id,source_document_id,source_occurrence_id,kind,provenance,statement,source_excerpt,start_byte,end_byte,created_at) VALUES (?,?,?,?,'user_asserted',?,?,?,?,?)").bind(&id).bind(&input.source_document_id).bind(&input.source_occurrence_id).bind(&input.kind).bind(&input.statement).bind(&input.source_excerpt).bind(input.start_byte).bind(input.end_byte).bind(&stamp).execute(&mut *tx).await?;
    for s in &input.linked_skill_ids {
        sqlx::query(
            "INSERT INTO evidence_claim_skill_links (claim_id,skill_id,created_at) VALUES (?,?,?)",
        )
        .bind(&id)
        .bind(s)
        .bind(&stamp)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    let r = sqlx::query("SELECT * FROM evidence_claims WHERE id=?")
        .bind(&id)
        .fetch_one(pool)
        .await?;
    claim_from(pool, &r).await
}

pub async fn review_claim(
    pool: &SqlitePool,
    input: ReviewEvidenceClaimInput,
) -> Result<EvidenceClaimDto, AppError> {
    if !matches!(
        input.decision.as_str(),
        "accept" | "edit" | "reject" | "exclude" | "defer" | "reopen"
    ) {
        return Err(AppError::Validation("invalid review decision".into()));
    }
    let mut tx = pool.begin().await?;
    let r = sqlx::query("SELECT * FROM evidence_claims WHERE id=?")
        .bind(&input.claim_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound("claim".into()))?;
    let state: String = r.get("review_state");
    let kind: String = r.get("kind");
    if input.portfolio_eligible
        && (!matches!(input.decision.as_str(), "accept" | "edit") || !eligible_kind(&kind))
    {
        return Err(AppError::Validation(
            "claim is not eligible for portfolio".into(),
        ));
    }
    let stamp = now();
    if input.decision == "reopen" {
        if !matches!(state.as_str(), "excluded" | "deferred") {
            return Err(AppError::InvalidState(
                "only excluded or deferred claims can be reopened".into(),
            ));
        }
        sqlx::query("UPDATE evidence_claims SET review_state='pending',portfolio_eligible=0,reviewed_at=NULL WHERE id=?")
            .bind(&input.claim_id).execute(&mut *tx).await?;
        tx.commit().await?;
        let row = sqlx::query("SELECT * FROM evidence_claims WHERE id=?")
            .bind(&input.claim_id)
            .fetch_one(pool)
            .await?;
        return claim_from(pool, &row).await;
    }
    if state != "pending" && !(state == "deferred" && input.decision == "defer") {
        return Err(AppError::InvalidState(
            "claim has already been reviewed".into(),
        ));
    }
    let result_id = if input.decision == "edit" {
        let statement = input
            .edited_statement
            .as_deref()
            .ok_or_else(|| AppError::Validation("edited statement is required".into()))?;
        nonblank(statement, "edited statement")?;
        let id = Uuid::new_v4().to_string();
        sqlx::query("UPDATE evidence_claims SET review_state='edited',reviewed_at=? WHERE id=?")
            .bind(&stamp)
            .bind(&input.claim_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO evidence_claims (id,source_document_id,source_occurrence_id,supersedes_claim_id,kind,provenance,statement,source_excerpt,start_byte,end_byte,confidence,review_state,portfolio_eligible,created_at,reviewed_at) SELECT ?,source_document_id,source_occurrence_id,id,kind,provenance,?,source_excerpt,start_byte,end_byte,confidence,'accepted',?,?,? FROM evidence_claims WHERE id=?").bind(&id).bind(statement).bind(if input.portfolio_eligible{1}else{0}).bind(&stamp).bind(&stamp).bind(&input.claim_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO evidence_claim_skill_links (claim_id,skill_id,created_at) SELECT ?,skill_id,? FROM evidence_claim_skill_links WHERE claim_id=?")
            .bind(&id).bind(&stamp).bind(&input.claim_id).execute(&mut *tx).await?;
        id
    } else if state == "deferred" {
        input.claim_id
    } else {
        let persisted = match input.decision.as_str() {
            "accept" => "accepted",
            "reject" => "rejected",
            "exclude" => "excluded",
            "defer" => "deferred",
            _ => unreachable!(),
        };
        sqlx::query("UPDATE evidence_claims SET review_state=?,portfolio_eligible=?,reviewed_at=? WHERE id=?")
            .bind(persisted)
            .bind(if input.decision == "accept" && input.portfolio_eligible { 1 } else { 0 })
            .bind(&stamp)
            .bind(&input.claim_id)
            .execute(&mut *tx)
            .await?;
        input.claim_id
    };
    tx.commit().await?;
    let r = sqlx::query("SELECT * FROM evidence_claims WHERE id=?")
        .bind(result_id)
        .fetch_one(pool)
        .await?;
    claim_from(pool, &r).await
}

pub async fn link_claim_activity(
    pool: &SqlitePool,
    input: ClaimActivityLinkInput,
) -> Result<(), AppError> {
    let stamp = now();
    let n=sqlx::query("INSERT INTO evidence_claim_activity_links (claim_id,activity_id,created_at) SELECT ?,?,? WHERE EXISTS(SELECT 1 FROM evidence_claims WHERE id=?) AND EXISTS(SELECT 1 FROM activities WHERE id=?) ON CONFLICT DO NOTHING").bind(&input.claim_id).bind(&input.activity_id).bind(&stamp).bind(&input.claim_id).bind(&input.activity_id).execute(pool).await?.rows_affected();
    if n == 0 {
        let ok: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM evidence_claims WHERE id=?")
            .bind(&input.claim_id)
            .fetch_one(pool)
            .await?;
        if ok == 0 {
            return Err(AppError::NotFound("claim".into()));
        }
        let ok: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM activities WHERE id=?")
            .bind(&input.activity_id)
            .fetch_one(pool)
            .await?;
        if ok == 0 {
            return Err(AppError::NotFound("activity".into()));
        }
    }
    Ok(())
}
fn project_from(r: &sqlx::sqlite::SqliteRow) -> ProjectDto {
    ProjectDto {
        id: r.get("id"),
        name: r.get("name"),
        summary: r.get("summary"),
        status: r.get("status"),
        evidence_count: r.get("evidence_count"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }
}
pub async fn create_project(
    pool: &SqlitePool,
    input: CreateProjectInput,
) -> Result<ProjectDto, AppError> {
    nonblank(&input.name, "project name")?;
    if !matches!(
        input.status.as_str(),
        "idea" | "active" | "paused" | "completed" | "archived"
    ) {
        return Err(AppError::Validation("invalid project status".into()));
    }
    let id = Uuid::new_v4().to_string();
    let stamp = now();
    sqlx::query(
        "INSERT INTO projects (id,name,summary,status,created_at,updated_at) VALUES (?,?,?,?,?,?)",
    )
    .bind(&id)
    .bind(&input.name)
    .bind(&input.summary)
    .bind(&input.status)
    .bind(&stamp)
    .bind(&stamp)
    .execute(pool)
    .await?;
    get_project(pool, &id).await
}
pub async fn get_project(pool: &SqlitePool, id: &str) -> Result<ProjectDto, AppError> {
    let r=sqlx::query("SELECT p.*,COUNT(l.claim_id) evidence_count FROM projects p LEFT JOIN project_evidence_links l ON l.project_id=p.id WHERE p.id=? GROUP BY p.id").bind(id).fetch_optional(pool).await?.ok_or_else(||AppError::NotFound("project".into()))?;
    Ok(project_from(&r))
}
pub async fn list_projects(pool: &SqlitePool) -> Result<Vec<ProjectDto>, AppError> {
    Ok(sqlx::query("SELECT p.*,COUNT(l.claim_id) evidence_count FROM projects p LEFT JOIN project_evidence_links l ON l.project_id=p.id GROUP BY p.id ORDER BY p.updated_at DESC").fetch_all(pool).await?.iter().map(project_from).collect())
}
async fn linkable(pool: &SqlitePool, id: &str) -> Result<(), AppError> {
    let r = sqlx::query("SELECT review_state FROM evidence_claims WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("claim".into()))?;
    let state: String = r.get("review_state");
    if state != "accepted" {
        return Err(AppError::InvalidState(
            "only accepted live claims can be linked to a project".into(),
        ));
    }
    Ok(())
}
pub async fn link_project_claim(
    pool: &SqlitePool,
    input: ProjectClaimLinkInput,
) -> Result<(), AppError> {
    get_project(pool, &input.project_id).await?;
    linkable(pool, &input.claim_id).await?;
    sqlx::query("INSERT INTO project_evidence_links (project_id,claim_id,created_at) VALUES (?,?,?) ON CONFLICT DO NOTHING").bind(&input.project_id).bind(&input.claim_id).bind(now()).execute(pool).await?;
    Ok(())
}
pub async fn unlink_project_claim(
    pool: &SqlitePool,
    input: ProjectClaimLinkInput,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM project_evidence_links WHERE project_id=? AND claim_id=?")
        .bind(input.project_id)
        .bind(input.claim_id)
        .execute(pool)
        .await?;
    Ok(())
}
pub async fn project_detail(pool: &SqlitePool, id: &str) -> Result<ProjectDetailDto, AppError> {
    let project = get_project(pool, id).await?;
    let mut claims = Vec::new();
    for row in sqlx::query("SELECT c.* FROM evidence_claims c JOIN project_evidence_links l ON l.claim_id=c.id WHERE l.project_id=? ORDER BY l.created_at")
        .bind(id).fetch_all(pool).await? { claims.push(claim_from(pool, &row).await?); }
    Ok(ProjectDetailDto { project, claims })
}
pub async fn linked_claim_activity(
    pool: &SqlitePool,
    input: ClaimActivityLinkInput,
) -> Result<EvidenceClaimDto, AppError> {
    let id = input.claim_id.clone();
    link_claim_activity(pool, input).await?;
    let row = sqlx::query("SELECT * FROM evidence_claims WHERE id=?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    claim_from(pool, &row).await
}
fn validate_claim_ids(ids: &[String]) -> Result<(), AppError> {
    if ids.is_empty() {
        return Err(AppError::Validation(
            "a portfolio draft needs at least one claim".into(),
        ));
    }
    let mut unique = HashSet::new();
    if ids.iter().any(|id| !unique.insert(id)) {
        return Err(AppError::Validation(
            "portfolio claim ids must be unique".into(),
        ));
    }
    Ok(())
}
fn body(title: &str, purpose: &str, claims: &[(String, String)]) -> String {
    let mut v = format!("# {title}\n\n{purpose}\n");
    for (kind, statement) in claims {
        v.push_str(&format!("\n## {kind}\n\n{statement}\n"));
    }
    v
}
async fn draft(pool: &SqlitePool, id: &str) -> Result<PortfolioDraftDto, AppError> {
    let r = sqlx::query("SELECT * FROM portfolio_drafts WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("portfolio draft".into()))?;
    let claim_ids = sqlx::query_scalar(
        "SELECT claim_id FROM portfolio_draft_items WHERE draft_id=? ORDER BY sort_order",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    Ok(PortfolioDraftDto {
        id: r.get("id"),
        title: r.get("title"),
        purpose: r.get("purpose"),
        body_markdown: r.get("body_markdown"),
        privacy_state: r.get("privacy_state"),
        claim_ids,
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    })
}
pub async fn create_draft(
    pool: &SqlitePool,
    input: CreatePortfolioDraftInput,
) -> Result<PortfolioDraftDto, AppError> {
    bounded(&input.title, "title", 200)?;
    bounded(&input.purpose, "purpose", 2_000)?;
    validate_claim_ids(&input.claim_ids)?;
    let mut tx = pool.begin().await?;
    let mut claims = Vec::new();
    for id in &input.claim_ids {
        let row = sqlx::query("SELECT kind,statement FROM evidence_claims WHERE id=? AND review_state='accepted' AND portfolio_eligible=1 AND kind NOT IN ('inference','personality_signal','knowledge','idea') AND EXISTS(SELECT 1 FROM source_documents s WHERE s.id=evidence_claims.source_document_id)")
            .bind(id).fetch_optional(&mut *tx).await?;
        let row = row.ok_or_else(|| {
            AppError::Validation(format!("claim is not eligible for portfolio: {id}"))
        })?;
        claims.push((row.get("kind"), row.get("statement")));
    }
    let id = Uuid::new_v4().to_string();
    let stamp = now();
    sqlx::query("INSERT INTO portfolio_drafts (id,title,purpose,body_markdown,privacy_state,created_at,updated_at) VALUES (?,?,?,?,'private',?,?)").bind(&id).bind(&input.title).bind(&input.purpose).bind(body(&input.title,&input.purpose,&claims)).bind(&stamp).bind(&stamp).execute(&mut *tx).await?;
    for (i, c) in input.claim_ids.iter().enumerate() {
        sqlx::query("INSERT INTO portfolio_draft_items (draft_id,claim_id,sort_order,created_at) VALUES (?,?,?,?)").bind(&id).bind(c).bind(i as i64).bind(&stamp).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    draft(pool, &id).await
}
pub async fn update_draft(
    pool: &SqlitePool,
    input: UpdatePortfolioDraftInput,
) -> Result<PortfolioDraftDto, AppError> {
    bounded(&input.title, "title", 200)?;
    bounded(&input.purpose, "purpose", 2_000)?;
    bounded(&input.body_markdown, "body markdown", 100_000)?;
    validate_claim_ids(&input.claim_ids)?;
    let mut tx = pool.begin().await?;
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM portfolio_drafts WHERE id=?")
        .bind(&input.draft_id)
        .fetch_one(&mut *tx)
        .await?;
    if exists != 1 {
        return Err(AppError::NotFound("portfolio draft".into()));
    }
    for id in &input.claim_ids {
        let eligible: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM evidence_claims WHERE id=? AND review_state='accepted' AND portfolio_eligible=1 AND kind NOT IN ('inference','personality_signal','knowledge','idea') AND EXISTS(SELECT 1 FROM source_documents s WHERE s.id=evidence_claims.source_document_id)")
            .bind(id).fetch_one(&mut *tx).await?;
        if eligible != 1 {
            return Err(AppError::Validation(format!(
                "claim is not eligible for portfolio: {id}"
            )));
        }
    }
    let stamp = now();
    sqlx::query("UPDATE portfolio_drafts SET title=?,purpose=?,body_markdown=?,privacy_state='private',updated_at=? WHERE id=?").bind(&input.title).bind(&input.purpose).bind(&input.body_markdown).bind(&stamp).bind(&input.draft_id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM portfolio_draft_items WHERE draft_id=?")
        .bind(&input.draft_id)
        .execute(&mut *tx)
        .await?;
    for (i, c) in input.claim_ids.iter().enumerate() {
        sqlx::query("INSERT INTO portfolio_draft_items (draft_id,claim_id,sort_order,created_at) VALUES (?,?,?,?)").bind(&input.draft_id).bind(c).bind(i as i64).bind(&stamp).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    draft(pool, &input.draft_id).await
}
pub async fn list_drafts(pool: &SqlitePool) -> Result<Vec<PortfolioDraftDto>, AppError> {
    let ids: Vec<String> =
        sqlx::query_scalar("SELECT id FROM portfolio_drafts ORDER BY updated_at DESC")
            .fetch_all(pool)
            .await?;
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        out.push(draft(pool, &id).await?);
    }
    Ok(out)
}

pub async fn library(
    pool: &SqlitePool,
    q: EvidenceLibraryQuery,
) -> Result<EvidenceLibraryDto, AppError> {
    let sources = sqlx::query("SELECT * FROM source_occurrences ORDER BY imported_at DESC")
        .fetch_all(pool)
        .await?
        .iter()
        .map(occurrence_from)
        .collect();
    let mut sql = "SELECT c.* FROM evidence_claims c".to_string();
    if q.project_id.is_some() {
        sql.push_str(" JOIN project_evidence_links pl ON pl.claim_id=c.id")
    }
    sql.push_str(" WHERE 1=1");
    if let Some(s) = q.review_state {
        sql.push_str(" AND c.review_state='");
        sql.push_str(&s.replace('\'', "''"));
        sql.push('\'')
    }
    if let Some(k) = q.kind {
        sql.push_str(" AND c.kind='");
        sql.push_str(&k.replace('\'', "''"));
        sql.push('\'')
    }
    if let Some(s) = q.search {
        sql.push_str(" AND (c.statement LIKE '%");
        sql.push_str(&s.replace('\'', "''"));
        sql.push_str("%' OR c.source_excerpt LIKE '%");
        sql.push_str(&s.replace('\'', "''"));
        sql.push_str("%')")
    }
    if let Some(p) = q.project_id {
        sql.push_str(" AND pl.project_id='");
        sql.push_str(&p.replace('\'', "''"));
        sql.push('\'')
    }
    sql.push_str(" ORDER BY c.created_at DESC");
    let mut claims = Vec::new();
    for r in sqlx::query(&sql).fetch_all(pool).await? {
        claims.push(claim_from(pool, &r).await?)
    }
    let c = EvidenceLibraryCountsDto {
        source_count: sqlx::query_scalar("SELECT COUNT(*) FROM source_documents")
            .fetch_one(pool)
            .await?,
        pending_claim_count: sqlx::query_scalar(
            "SELECT COUNT(*) FROM evidence_claims WHERE review_state='pending'",
        )
        .fetch_one(pool)
        .await?,
        accepted_claim_count: sqlx::query_scalar(
            "SELECT COUNT(*) FROM evidence_claims WHERE review_state='accepted'",
        )
        .fetch_one(pool)
        .await?,
        inference_count: sqlx::query_scalar(
            "SELECT COUNT(*) FROM evidence_claims WHERE kind='inference'",
        )
        .fetch_one(pool)
        .await?,
        project_count: sqlx::query_scalar("SELECT COUNT(*) FROM projects")
            .fetch_one(pool)
            .await?,
        private_draft_count: sqlx::query_scalar(
            "SELECT COUNT(*) FROM portfolio_drafts WHERE privacy_state='private'",
        )
        .fetch_one(pool)
        .await?,
    };
    Ok(EvidenceLibraryDto {
        sources,
        claims,
        counts: c,
    })
}
pub async fn source_detail(
    pool: &SqlitePool,
    id: &str,
) -> Result<SourceDocumentDetailDto, AppError> {
    let r = sqlx::query("SELECT * FROM source_documents WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("source document".into()))?;
    let occ = sqlx::query(
        "SELECT * FROM source_occurrences WHERE source_document_id=? ORDER BY imported_at DESC",
    )
    .bind(id)
    .fetch_all(pool)
    .await?
    .iter()
    .map(occurrence_from)
    .collect();
    let mut claims = Vec::new();
    for r in sqlx::query(
        "SELECT * FROM evidence_claims WHERE source_document_id=? ORDER BY created_at DESC",
    )
    .bind(id)
    .fetch_all(pool)
    .await?
    {
        claims.push(claim_from(pool, &r).await?)
    }
    Ok(SourceDocumentDetailDto {
        document: source_from(&r),
        occurrences: occ,
        claims,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::Database;

    async fn db() -> (tempfile::NamedTempFile, Database) {
        let file = tempfile::NamedTempFile::new().unwrap();
        let db = Database::open(file.path()).await.unwrap();
        (file, db)
    }

    fn claim(source_document_id: String, excerpt: &str) -> CreateEvidenceClaimInput {
        CreateEvidenceClaimInput {
            source_document_id,
            source_occurrence_id: None,
            kind: "experience".into(),
            statement: "Implemented the feature".into(),
            source_excerpt: excerpt.into(),
            start_byte: None,
            end_byte: None,
            linked_skill_ids: vec!["technical.system_design".into()],
        }
    }

    #[tokio::test]
    async fn evidence_operations_preserve_sources_and_do_not_touch_growth_evidence() {
        let (_file, db) = db().await;
        let pool = db.pool();
        let one = store_source(pool, "paste", "one", None, "日本語で設計を実装した")
            .await
            .unwrap();
        let two = store_source(pool, "paste", "two", None, "日本語で設計を実装した")
            .await
            .unwrap();
        assert!(two.duplicate_content);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM source_documents")
                .fetch_one(pool)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM source_occurrences")
                .fetch_one(pool)
                .await
                .unwrap(),
            2
        );
        let long_text = "x".repeat(25_000);
        assert!(
            store_source(pool, "paste", "long", None, &long_text)
                .await
                .is_ok()
        );
        assert!(
            store_source(pool, "paste", "nul", None, "a\0b")
                .await
                .is_err()
        );
        assert!(
            store_source(
                pool,
                "paste",
                "too-large",
                None,
                &"x".repeat(1024 * 1024 + 1)
            )
            .await
            .is_err()
        );
        let mut invalid = claim(one.document.id.clone(), "設計");
        invalid.start_byte = Some(1);
        invalid.end_byte = Some(7);
        assert!(
            create_claim(pool, invalid).await.is_err(),
            "non-UTF8 anchor is rejected"
        );
        let mut mismatch = claim(one.document.id.clone(), "not in source");
        mismatch.start_byte = Some(0);
        mismatch.end_byte = Some(1);
        assert!(
            create_claim(pool, mismatch).await.is_err(),
            "mismatched excerpt is rejected"
        );
        let original = create_claim(pool, claim(one.document.id.clone(), "設計を実装"))
            .await
            .unwrap();
        let successor = review_claim(
            pool,
            ReviewEvidenceClaimInput {
                claim_id: original.id.clone(),
                decision: "edit".into(),
                edited_statement: Some("Implemented a Japanese design".into()),
                portfolio_eligible: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(successor.review_state, "accepted");
        assert_eq!(
            successor.supersedes_claim_id.as_deref(),
            Some(original.id.as_str())
        );
        let original_row =
            sqlx::query("SELECT statement, review_state FROM evidence_claims WHERE id=?")
                .bind(&original.id)
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(
            original_row.get::<String, _>("statement"),
            "Implemented the feature"
        );
        assert_eq!(original_row.get::<String, _>("review_state"), "edited");
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT content_text FROM source_documents WHERE id=?")
                .bind(&one.document.id)
                .fetch_one(pool)
                .await
                .unwrap(),
            "日本語で設計を実装した"
        );
        assert!(
            review_claim(
                pool,
                ReviewEvidenceClaimInput {
                    claim_id: original.id.clone(),
                    decision: "accept".into(),
                    edited_statement: None,
                    portfolio_eligible: false
                }
            )
            .await
            .is_err()
        );
        let pending = create_claim(pool, claim(one.document.id.clone(), "設計を実装"))
            .await
            .unwrap();
        for (action, state) in [
            ("defer", "deferred"),
            ("reject", "rejected"),
            ("exclude", "excluded"),
        ] {
            let target = if action == "defer" {
                pending.id.clone()
            } else {
                create_claim(pool, claim(one.document.id.clone(), "設計を実装"))
                    .await
                    .unwrap()
                    .id
            };
            let reviewed = review_claim(
                pool,
                ReviewEvidenceClaimInput {
                    claim_id: target,
                    decision: action.into(),
                    edited_statement: None,
                    portfolio_eligible: false,
                },
            )
            .await
            .unwrap();
            assert_eq!(reviewed.review_state, state);
            if action == "exclude" {
                let reopened = review_claim(
                    pool,
                    ReviewEvidenceClaimInput {
                        claim_id: reviewed.id,
                        decision: "reopen".into(),
                        edited_statement: None,
                        portfolio_eligible: false,
                    },
                )
                .await
                .unwrap();
                assert_eq!(reopened.review_state, "pending");
            }
        }
        let project = create_project(
            pool,
            CreateProjectInput {
                name: "Levelog".into(),
                summary: "".into(),
                status: "active".into(),
            },
        )
        .await
        .unwrap();
        let directly_accepted = create_claim(pool, claim(one.document.id.clone(), "設計を実装"))
            .await
            .unwrap();
        let directly_accepted = review_claim(
            pool,
            ReviewEvidenceClaimInput {
                claim_id: directly_accepted.id,
                decision: "accept".into(),
                edited_statement: None,
                portfolio_eligible: true,
            },
        )
        .await
        .unwrap();
        link_project_claim(
            pool,
            ProjectClaimLinkInput {
                project_id: project.id.clone(),
                claim_id: directly_accepted.id.clone(),
            },
        )
        .await
        .unwrap();
        assert_eq!(
            get_project(pool, &project.id).await.unwrap().evidence_count,
            1
        );
        unlink_project_claim(
            pool,
            ProjectClaimLinkInput {
                project_id: project.id.clone(),
                claim_id: directly_accepted.id.clone(),
            },
        )
        .await
        .unwrap();
        assert_eq!(
            get_project(pool, &project.id).await.unwrap().evidence_count,
            0
        );
        let relation = create_relation(
            pool,
            CreateEvidenceRelationInput {
                from_claim_id: successor.id.clone(),
                to_claim_id: directly_accepted.id.clone(),
                relation_type: "supports".into(),
            },
        )
        .await
        .unwrap();
        let replayed = create_relation(
            pool,
            CreateEvidenceRelationInput {
                from_claim_id: successor.id.clone(),
                to_claim_id: directly_accepted.id.clone(),
                relation_type: "supports".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(relation.id, replayed.id);
        assert_eq!(list_relations(pool).await.unwrap().len(), 1);
        assert!(
            create_relation(
                pool,
                CreateEvidenceRelationInput {
                    from_claim_id: successor.id.clone(),
                    to_claim_id: pending.id.clone(),
                    relation_type: "related".into()
                }
            )
            .await
            .is_err()
        );
        delete_relation(pool, &relation.id).await.unwrap();
        assert!(list_relations(pool).await.unwrap().is_empty());
        assert!(
            create_draft(
                pool,
                CreatePortfolioDraftInput {
                    title: "bad".into(),
                    purpose: "test".into(),
                    claim_ids: vec![pending.id]
                }
            )
            .await
            .is_err()
        );
        let inference = create_claim(
            pool,
            CreateEvidenceClaimInput {
                kind: "inference".into(),
                ..claim(one.document.id.clone(), "設計を実装")
            },
        )
        .await
        .unwrap();
        assert!(
            review_claim(
                pool,
                ReviewEvidenceClaimInput {
                    claim_id: inference.id,
                    decision: "accept".into(),
                    edited_statement: None,
                    portfolio_eligible: true
                }
            )
            .await
            .is_err()
        );
        let inference_ok = create_claim(
            pool,
            CreateEvidenceClaimInput {
                kind: "inference".into(),
                ..claim(one.document.id.clone(), "設計を実装")
            },
        )
        .await
        .unwrap();
        assert_eq!(
            review_claim(
                pool,
                ReviewEvidenceClaimInput {
                    claim_id: inference_ok.id,
                    decision: "accept".into(),
                    edited_statement: None,
                    portfolio_eligible: false
                }
            )
            .await
            .unwrap()
            .review_state,
            "accepted"
        );
        let before_failed_draft: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM portfolio_drafts")
            .fetch_one(pool)
            .await
            .unwrap();
        assert!(
            create_draft(
                pool,
                CreatePortfolioDraftInput {
                    title: "duplicate".into(),
                    purpose: "test".into(),
                    claim_ids: vec![successor.id.clone(), successor.id.clone()]
                }
            )
            .await
            .is_err()
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM portfolio_drafts")
                .fetch_one(pool)
                .await
                .unwrap(),
            before_failed_draft
        );
        let draft = create_draft(
            pool,
            CreatePortfolioDraftInput {
                title: "good".into(),
                purpose: "test".into(),
                claim_ids: vec![successor.id],
            },
        )
        .await
        .unwrap();
        assert_eq!(draft.privacy_state, "private");
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM xp_events")
                .fetch_one(pool)
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM skill_observations")
                .fetch_one(pool)
                .await
                .unwrap(),
            0
        );
    }
}
