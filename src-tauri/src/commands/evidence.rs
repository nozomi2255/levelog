use crate::{application::evidence, dto::*, error::AppError, state::AppState};
use tauri::State;
#[tauri::command]
pub async fn import_pasted_source(
    state: State<'_, AppState>,
    input: ImportPastedSourceInput,
) -> Result<SourceImportResult, AppError> {
    let value = evidence::store_source(
        state.db.pool(),
        "paste",
        &input.display_name,
        None,
        &input.content_text,
    )
    .await?;
    Ok(SourceImportResult {
        imported: vec![value],
        failures: vec![],
    })
}
#[tauri::command]
pub async fn list_evidence_library(
    state: State<'_, AppState>,
    input: EvidenceLibraryQuery,
) -> Result<EvidenceLibraryDto, AppError> {
    evidence::library(state.db.pool(), input).await
}
#[tauri::command]
pub async fn get_evidence_source(
    state: State<'_, AppState>,
    source_id: String,
) -> Result<SourceDocumentDetailDto, AppError> {
    evidence::source_detail(state.db.pool(), &source_id).await
}
#[tauri::command]
pub async fn create_evidence_claim(
    state: State<'_, AppState>,
    input: CreateEvidenceClaimInput,
) -> Result<EvidenceClaimDto, AppError> {
    evidence::create_claim(state.db.pool(), input).await
}
#[tauri::command]
pub async fn review_evidence_claim(
    state: State<'_, AppState>,
    input: ReviewEvidenceClaimInput,
) -> Result<EvidenceClaimDto, AppError> {
    evidence::review_claim(state.db.pool(), input).await
}
#[tauri::command]
pub async fn list_evidence_relations(
    state: State<'_, AppState>,
) -> Result<Vec<EvidenceRelationDto>, AppError> {
    evidence::list_relations(state.db.pool()).await
}
#[tauri::command]
pub async fn create_evidence_relation(
    state: State<'_, AppState>,
    input: CreateEvidenceRelationInput,
) -> Result<EvidenceRelationDto, AppError> {
    evidence::create_relation(state.db.pool(), input).await
}
#[tauri::command]
pub async fn delete_evidence_relation(
    state: State<'_, AppState>,
    relation_id: String,
) -> Result<(), AppError> {
    evidence::delete_relation(state.db.pool(), &relation_id).await
}
#[tauri::command]
pub async fn link_claim_to_activity(
    state: State<'_, AppState>,
    input: ClaimActivityLinkInput,
) -> Result<EvidenceClaimDto, AppError> {
    evidence::linked_claim_activity(state.db.pool(), input).await
}
#[tauri::command]
pub async fn create_project(
    state: State<'_, AppState>,
    input: CreateProjectInput,
) -> Result<ProjectDto, AppError> {
    evidence::create_project(state.db.pool(), input).await
}
#[tauri::command]
pub async fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectDto>, AppError> {
    evidence::list_projects(state.db.pool()).await
}
#[tauri::command]
pub async fn get_project(
    state: State<'_, AppState>,
    project_id: String,
) -> Result<ProjectDetailDto, AppError> {
    evidence::project_detail(state.db.pool(), &project_id).await
}
#[tauri::command]
pub async fn link_claim_to_project(
    state: State<'_, AppState>,
    input: ProjectClaimLinkInput,
) -> Result<ProjectDetailDto, AppError> {
    let project_id = input.project_id.clone();
    evidence::link_project_claim(state.db.pool(), input).await?;
    evidence::project_detail(state.db.pool(), &project_id).await
}
#[tauri::command]
pub async fn unlink_claim_from_project(
    state: State<'_, AppState>,
    input: ProjectClaimLinkInput,
) -> Result<ProjectDetailDto, AppError> {
    let project_id = input.project_id.clone();
    evidence::unlink_project_claim(state.db.pool(), input).await?;
    evidence::project_detail(state.db.pool(), &project_id).await
}
#[tauri::command]
pub async fn create_portfolio_draft(
    state: State<'_, AppState>,
    input: CreatePortfolioDraftInput,
) -> Result<PortfolioDraftDto, AppError> {
    evidence::create_draft(state.db.pool(), input).await
}
#[tauri::command]
pub async fn update_portfolio_draft(
    state: State<'_, AppState>,
    input: UpdatePortfolioDraftInput,
) -> Result<PortfolioDraftDto, AppError> {
    evidence::update_draft(state.db.pool(), input).await
}
#[tauri::command]
pub async fn list_portfolio_drafts(
    state: State<'_, AppState>,
) -> Result<Vec<PortfolioDraftDto>, AppError> {
    evidence::list_drafts(state.db.pool()).await
}
