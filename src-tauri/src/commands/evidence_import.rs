//! Native-picker evidence intake and explicitly previewed Codex extraction.
use std::path::{Path, PathBuf};

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tokio::sync::watch;
use uuid::Uuid;

use crate::{
    application::evidence::store_source,
    dto::{
        CodexConnectionStatus, EvidenceAnalysisJobDto, EvidenceAnalysisPreviewDto,
        EvidenceExtractionOutput, RedactionFindingDto, SourceImportFailureDto, SourceImportResult,
        StartEvidenceAnalysisInput,
    },
    error::AppError,
    infrastructure::{
        codex::{
            CodexClient, CodexError, CodexJsonOutput, EVIDENCE_SCHEMA_VERSION, TIMEOUT,
            TokioProcessRunner,
        },
        source_import::{self, MAX_FILES},
    },
    state::AppState,
};

const MAX_PAYLOAD_BYTES: usize = 512 * 1024;
const FIXED_INSTRUCTION: &str = "Extract only source-grounded candidates.";

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[tauri::command]
pub async fn pick_and_import_sources(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SourceImportResult, AppError> {
    let selected = tokio::task::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("Text", &["md", "markdown", "txt"])
            .blocking_pick_files()
    })
    .await
    .map_err(|error| AppError::Internal(error.to_string()))?
    .unwrap_or_default();
    let mut imported = Vec::new();
    let mut failures = Vec::new();
    let mut total = 0_u64;
    for (index, file) in selected.into_iter().enumerate() {
        if index >= MAX_FILES {
            failures.push(SourceImportFailureDto {
                display_name: "追加の選択ファイル".into(),
                message: format!("一度に取り込めるのは {MAX_FILES} 件までです"),
            });
            continue;
        }
        let path = match file.into_path() {
            Ok(path) => path,
            Err(_) => {
                failures.push(SourceImportFailureDto {
                    display_name: "選択ファイル".into(),
                    message: "ローカルパスへ変換できません".into(),
                });
                continue;
            }
        };
        match source_import::validate_file(&path, total) {
            Ok(source) => match store_source(
                state.db.pool(),
                &source.kind,
                &source.display_name,
                Some(&source.original_path.display().to_string()),
                &source.content,
            )
            .await
            {
                Ok(value) => {
                    total += source.content.len() as u64;
                    imported.push(value);
                }
                Err(error) => failures.push(SourceImportFailureDto {
                    display_name: source.display_name,
                    message: error.to_string(),
                }),
            },
            Err(message) => failures.push(SourceImportFailureDto {
                display_name: display_name(&path),
                message,
            }),
        }
    }
    Ok(SourceImportResult { imported, failures })
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("選択ファイル")
        .to_owned()
}

fn secret_findings(content: &str) -> Vec<RedactionFindingDto> {
    let lower = content.to_ascii_lowercase();
    let mut findings = Vec::new();
    for (needle, label) in [
        ("begin private key", "private_key"),
        ("begin rsa private key", "private_key"),
        ("password", "password"),
        ("api_key", "api_key"),
        ("apikey", "api_key"),
        ("token", "token"),
    ] {
        let mut cursor = 0;
        while let Some(offset) = lower[cursor..].find(needle) {
            let start = cursor + offset;
            let end = (start + needle.len() + 128).min(content.len());
            findings.push(RedactionFindingDto {
                kind: label.into(),
                start_byte: start as i64,
                end_byte: end as i64,
            });
            cursor = start + needle.len();
        }
    }
    // Conservative ASCII email detector: it emits only positions and a label, never the value.
    let bytes = content.as_bytes();
    for at in bytes
        .iter()
        .enumerate()
        .filter_map(|(i, byte)| (*byte == b'@').then_some(i))
    {
        let left = bytes[..at]
            .iter()
            .rev()
            .take_while(|b| {
                b.is_ascii_alphanumeric() || matches!(**b, b'.' | b'_' | b'%' | b'+' | b'-')
            })
            .count();
        let right = bytes[at + 1..]
            .iter()
            .take_while(|b| b.is_ascii_alphanumeric() || matches!(**b, b'.' | b'-'))
            .count();
        if left > 0 && right >= 3 && bytes[at + 1..at + 1 + right].contains(&b'.') {
            findings.push(RedactionFindingDto {
                kind: "email".into(),
                start_byte: (at - left) as i64,
                end_byte: (at + 1 + right) as i64,
            });
        }
    }
    findings
}

fn candidate_matches_source(
    content: &str,
    excerpt: &str,
    start: Option<i64>,
    end: Option<i64>,
) -> bool {
    match (start, end) {
        (Some(start), Some(end)) if start >= 0 && end >= start => {
            let (start, end) = (start as usize, end as usize);
            end <= content.len()
                && content.is_char_boundary(start)
                && content.is_char_boundary(end)
                && &content[start..end] == excerpt
        }
        (None, None) => content.contains(excerpt),
        _ => false,
    }
}

fn preview_payload(id: &str, sha256: &str, content: String) -> serde_json::Value {
    serde_json::json!({"source":{"id":id,"sha256":sha256,"content":content,"trust":"untrusted_data"},"instruction":FIXED_INSTRUCTION})
}

fn validate_editable_payload(
    value: serde_json::Value,
    source_id: &str,
    source_hash: &str,
) -> Result<String, AppError> {
    let object = value
        .as_object()
        .ok_or_else(|| AppError::Validation("送信JSONはオブジェクトにしてください".into()))?;
    if object.len() != 2
        || !object.contains_key("source")
        || object.get("instruction").and_then(|v| v.as_str()) != Some(FIXED_INSTRUCTION)
    {
        return Err(AppError::Validation(
            "プレビューの固定フィールドまたは追加フィールドは変更できません".into(),
        ));
    }
    let source = object
        .get("source")
        .and_then(|v| v.as_object())
        .ok_or_else(|| AppError::Validation("source が正しくありません".into()))?;
    if source.len() != 4
        || source.get("id").and_then(|v| v.as_str()) != Some(source_id)
        || source.get("sha256").and_then(|v| v.as_str()) != Some(source_hash)
        || source.get("trust").and_then(|v| v.as_str()) != Some("untrusted_data")
        || source.get("content").and_then(|v| v.as_str()).is_none()
    {
        return Err(AppError::Validation(
            "source の固定フィールドまたは追加フィールドは変更できません".into(),
        ));
    }
    let serialized = serde_json::to_string_pretty(&serde_json::Value::Object(object.clone()))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    if serialized.len() > MAX_PAYLOAD_BYTES {
        return Err(AppError::Validation(
            "送信内容が512 KiBを超えています".into(),
        ));
    }
    Ok(serialized)
}

#[tauri::command]
pub async fn get_evidence_analysis_preview(
    state: State<'_, AppState>,
    source_id: String,
) -> Result<EvidenceAnalysisPreviewDto, AppError> {
    let (id, content, hash) = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, content_text, content_sha256 FROM source_documents WHERE id = ?",
    )
    .bind(&source_id)
    .fetch_optional(state.db.pool())
    .await?
    .ok_or_else(|| AppError::NotFound("source document".into()))?;
    let text = serde_json::to_string_pretty(&preview_payload(&id, &hash, content.clone()))
        .map_err(|e| AppError::Internal(e.to_string()))?;
    if text.len() > MAX_PAYLOAD_BYTES {
        return Err(AppError::Validation(
            "送信内容が512 KiBを超えるため解析できません。原文は保存されています。".into(),
        ));
    }
    let findings = secret_findings(&content);
    let needs_review = !findings.is_empty();
    Ok(EvidenceAnalysisPreviewDto {
        source_id,
        submitted_payload: text,
        cloud_inference_notice:
            "この正確な内容はCodexの推論先へ送信されます。秘密情報を確認・編集してください。".into(),
        redaction_findings: findings,
        needs_review,
    })
}

async fn job(state: &AppState, id: &str) -> Result<EvidenceAnalysisJobDto, AppError> {
    let row = sqlx::query_as::<_, (String, String, String, Option<String>, String, Option<String>)>("SELECT id,source_document_id,status,error_message,created_at,completed_at FROM evidence_analysis_jobs WHERE id=?").bind(id).fetch_one(state.db.pool()).await?;
    Ok(EvidenceAnalysisJobDto {
        id: row.0,
        source_document_id: row.1,
        status: row.2,
        error_message: row.3,
        created_at: row.4,
        completed_at: row.5,
    })
}

async fn configured_codex_path(state: &AppState) -> Result<PathBuf, CodexError> {
    let value: Option<String> =
        sqlx::query_scalar("SELECT value_json FROM app_settings WHERE key = 'codex_connection'")
            .fetch_optional(state.db.pool())
            .await
            .map_err(|e| CodexError::Process(e.to_string()))?;
    let connection: CodexConnectionStatus = value
        .as_deref()
        .ok_or_else(|| CodexError::Process("Codex接続が設定されていません".into()))
        .and_then(|raw| {
            serde_json::from_str(raw).map_err(|e| CodexError::Process(e.to_string()))
        })?;
    let path = PathBuf::from(connection.path);
    if !connection.available || !connection.authenticated || !path.is_absolute() {
        return Err(CodexError::Process(
            "安全なCodex接続が設定されていません".into(),
        ));
    }
    Ok(path)
}

async fn persist_evidence_success(
    state: &AppState,
    run_id: &str,
    source_content: &str,
    output: CodexJsonOutput<EvidenceExtractionOutput>,
) -> Result<(), String> {
    let raw_json = output.raw_json;
    let mut transaction = state
        .db
        .pool()
        .begin()
        .await
        .map_err(|error| error.to_string())?;
    let active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM evidence_analysis_jobs WHERE id=? AND status='running'",
    )
    .bind(run_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|error| error.to_string())?;
    if active != 1 {
        return Ok(());
    }
    if !output.parsed.candidates.iter().all(|candidate| {
        candidate_matches_source(
            source_content,
            &candidate.source_excerpt,
            candidate.start_byte,
            candidate.end_byte,
        )
    }) {
        sqlx::query("UPDATE evidence_analysis_jobs SET status='failed',error_message='source excerpt does not match immutable source',raw_result_json=?,completed_at=? WHERE id=? AND status='running'").bind(raw_json).bind(now()).bind(run_id).execute(&mut *transaction).await.map_err(|error| error.to_string())?;
        transaction
            .commit()
            .await
            .map_err(|error| error.to_string())?;
        return Ok(());
    }
    for candidate in output.parsed.candidates {
        sqlx::query("INSERT INTO evidence_claims (id,source_document_id,kind,provenance,statement,source_excerpt,start_byte,end_byte,confidence,review_state,portfolio_eligible,created_at) SELECT ?,source_document_id,?,?,?,?,?,?,?,'pending',0,? FROM evidence_analysis_jobs WHERE id=?").bind(Uuid::new_v4().to_string()).bind(candidate.kind).bind(candidate.provenance).bind(candidate.statement).bind(candidate.source_excerpt).bind(candidate.start_byte).bind(candidate.end_byte).bind(candidate.confidence).bind(now()).bind(run_id).execute(&mut *transaction).await.map_err(|error| error.to_string())?;
    }
    let updated = sqlx::query("UPDATE evidence_analysis_jobs SET status='succeeded',raw_result_json=?,completed_at=? WHERE id=? AND status='running'").bind(raw_json).bind(now()).bind(run_id).execute(&mut *transaction).await.map_err(|error| error.to_string())?;
    if updated.rows_affected() != 1 {
        return Err("analysis job was cancelled before it could be committed".into());
    }
    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}

async fn save_evidence_failure(
    state: &AppState,
    run_id: &str,
    message: String,
    raw: Option<String>,
) {
    let _ = sqlx::query("UPDATE evidence_analysis_jobs SET status='failed',error_message=?,raw_result_json=COALESCE(?, raw_result_json),completed_at=? WHERE id=? AND status='running'").bind(message).bind(raw).bind(now()).bind(run_id).execute(state.db.pool()).await;
}

#[tauri::command]
pub async fn start_evidence_analysis(
    state: State<'_, AppState>,
    input: StartEvidenceAnalysisInput,
) -> Result<EvidenceAnalysisJobDto, AppError> {
    let (source_hash, source_content) = sqlx::query_as::<_, (String, String)>(
        "SELECT content_sha256, content_text FROM source_documents WHERE id=?",
    )
    .bind(&input.source_document_id)
    .fetch_optional(state.db.pool())
    .await?
    .ok_or_else(|| AppError::NotFound("source document".into()))?;
    let submitted = validate_editable_payload(
        serde_json::from_str(&input.submitted_payload)
            .map_err(|e| AppError::Validation(format!("送信JSONが正しくありません: {e}")))?,
        &input.source_document_id,
        &source_hash,
    )?;
    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO evidence_analysis_jobs (id,source_document_id,status,submitted_payload,provider,prompt_version,schema_version,created_at) VALUES (?,?,'running',?,'codex-cli',?,?,?)").bind(&id).bind(&input.source_document_id).bind(&submitted).bind(EVIDENCE_SCHEMA_VERSION).bind(EVIDENCE_SCHEMA_VERSION).bind(now()).execute(state.db.pool()).await?;
    let (sender, receiver) = watch::channel(false);
    state
        .evidence_analysis_cancellations
        .lock()
        .await
        .insert(id.clone(), sender);
    let app = state.inner().clone();
    let run_id = id.clone();
    let retry_payload = submitted.clone();
    tauri::async_runtime::spawn(async move {
        let execution = async {
            let mut queued_cancel = receiver.clone();
            let _permit = tokio::select! {
                permit = app.codex_semaphore.clone().acquire_owned() => permit.map_err(|_| CodexError::Process("Codex実行キューを開始できませんでした".into()))?,
                _ = queued_cancel.changed() => return Err(CodexError::Cancelled),
            };
            let path = configured_codex_path(&app).await?;
            let client = CodexClient::new(path, TokioProcessRunner)?;
            let connection = client.probe().await?;
            let updated = sqlx::query(
                "UPDATE evidence_analysis_jobs SET codex_version=? WHERE id=? AND status='running'",
            )
            .bind(connection.version)
            .bind(&run_id)
            .execute(app.db.pool())
            .await
            .map_err(|error| CodexError::Process(error.to_string()))?;
            if updated.rows_affected() != 1 {
                return Err(CodexError::Cancelled);
            }
            let first = client.analyze_evidence(submitted, receiver.clone()).await;
            if first.as_ref().is_err_and(CodexError::is_schema_retryable) {
                client.analyze_evidence(retry_payload, receiver).await
            } else {
                first
            }
        };
        let result = tokio::time::timeout(TIMEOUT, execution)
            .await
            .unwrap_or(Err(CodexError::TimedOut));
        match result {
            Ok(output) => {
                let raw = output.raw_json.clone();
                if let Err(message) =
                    persist_evidence_success(&app, &run_id, &source_content, output).await
                {
                    save_evidence_failure(&app, &run_id, message, Some(raw)).await;
                }
            }
            Err(error) => {
                let status = if matches!(error, CodexError::Cancelled) {
                    "cancelled"
                } else {
                    "failed"
                };
                let raw = error.raw_output().map(str::to_owned);
                let _ = sqlx::query("UPDATE evidence_analysis_jobs SET status=?,error_message=?,raw_result_json=COALESCE(?, raw_result_json),completed_at=? WHERE id=? AND status IN ('pending','running')").bind(status).bind(error.to_string()).bind(raw).bind(now()).bind(&run_id).execute(app.db.pool()).await;
            }
        }
        app.evidence_analysis_cancellations
            .lock()
            .await
            .remove(&run_id);
    });
    job(&state, &id).await
}

#[tauri::command]
pub async fn get_evidence_analysis(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<EvidenceAnalysisJobDto, AppError> {
    job(&state, &job_id).await
}
#[tauri::command]
pub async fn cancel_evidence_analysis(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<EvidenceAnalysisJobDto, AppError> {
    if let Some(sender) = state
        .evidence_analysis_cancellations
        .lock()
        .await
        .remove(&job_id)
    {
        let _ = sender.send(true);
    }
    sqlx::query("UPDATE evidence_analysis_jobs SET status='cancelled',error_message='ユーザーが解析をキャンセルしました',completed_at=? WHERE id=? AND status IN ('pending','running')").bind(now()).bind(&job_id).execute(state.db.pool()).await?;
    job(&state, &job_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editable_payload_only_permits_content_changes() {
        let valid = preview_payload("source-1", "hash-1", "redacted".into());
        assert!(validate_editable_payload(valid, "source-1", "hash-1").is_ok());
        let invalid = serde_json::json!({
            "source": {"id":"source-1", "sha256":"hash-1", "content":"x", "trust":"untrusted_data"},
            "instruction": FIXED_INSTRUCTION,
            "extra": true
        });
        assert!(validate_editable_payload(invalid, "source-1", "hash-1").is_err());
    }

    #[test]
    fn scanner_labels_without_returning_secret_value() {
        let findings = secret_findings("password=do-not-return-me a@b.example");
        assert!(findings.iter().any(|finding| finding.kind == "password"));
        assert!(
            findings
                .iter()
                .all(|finding| finding.end_byte > finding.start_byte)
        );
    }
}
