import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../../lib/api";
import { EvidenceLibraryPage, ImportEvidencePage } from "./EvidencePages";

vi.mock("../../lib/api", () => ({ api: { listEvidenceLibrary: vi.fn(), listEvidenceRelations: vi.fn(), createEvidenceRelation: vi.fn(), deleteEvidenceRelation: vi.fn(), reviewEvidenceClaim: vi.fn(), importPastedSource: vi.fn(), pickAndImportSources: vi.fn(), getEvidenceAnalysisPreview: vi.fn(), startEvidenceAnalysis: vi.fn(), getEvidenceAnalysis: vi.fn(), cancelEvidenceAnalysis: vi.fn() } }));
const renderPage = () => render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}><MemoryRouter><EvidenceLibraryPage /></MemoryRouter></QueryClientProvider>);
const renderImport = () => render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}><MemoryRouter><ImportEvidencePage /></MemoryRouter></QueryClientProvider>);

describe("EvidenceLibraryPage", () => {
  const baseClaim = { sourceDocumentId: "source-1", sourceOccurrenceId: "occ-1", supersedesClaimId: null, kind: "experience" as const, provenance: "user_asserted" as const, sourceExcerpt: "excerpt", startByte: null, endByte: null, confidence: 1, portfolioEligible: true, linkedSkillIds: [], createdAt: "now", reviewedAt: "now" };
  beforeEach(() => vi.mocked(api.listEvidenceLibrary).mockResolvedValue({
    counts: { sourceCount: 1, pendingClaimCount: 1, acceptedClaimCount: 1, inferenceCount: 0, projectCount: 0, privateDraftCount: 0 },
    sources: [{ id: "occ-1", sourceDocumentId: "source-1", sourceKind: "markdown", displayName: "project.md", originalPath: null, importedAt: "2026-07-20T00:00:00Z" }],
    claims: [{ id: "claim-1", sourceDocumentId: "source-1", sourceOccurrenceId: "occ-1", supersedesClaimId: null, kind: "knowledge", provenance: "import_extracted", statement: "Reactについて調査した", sourceExcerpt: "React notes", startByte: null, endByte: null, confidence: 0.7, reviewState: "pending", portfolioEligible: false, linkedSkillIds: [], createdAt: "now", reviewedAt: null }],
  }));
  beforeEach(() => vi.mocked(api.listEvidenceRelations).mockResolvedValue([]));
  afterEach(cleanup);
  it("keeps source, proposal, and knowledge-evidence warning distinct", async () => {
    renderPage();
    expect(await screen.findByRole("heading", { name: "エビデンスライブラリ", level: 1 })).toBeInTheDocument();
    expect(screen.getByText("LOCAL ORIGINAL")).toBeInTheDocument();
    expect(screen.getByText("知識メモは、能力を実務で発揮した根拠にはなりません。")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "記録を取り込む" })).toHaveAttribute("href", "/memory/import");
    expect(screen.getByRole("link", { name: "プロジェクト" })).toHaveAttribute("href", "/projects");
    expect(screen.getByRole("link", { name: "ポートフォリオ" })).toHaveAttribute("href", "/portfolio");
  });
  it("creates and removes explicit relations between accepted claims", async () => {
    const user = userEvent.setup();
    vi.mocked(api.listEvidenceLibrary).mockResolvedValue({ counts: { sourceCount: 1, pendingClaimCount: 0, acceptedClaimCount: 2, inferenceCount: 0, projectCount: 0, privateDraftCount: 0 }, sources: [], claims: [{ ...baseClaim, id: "a", statement: "設計した", reviewState: "accepted" }, { ...baseClaim, id: "b", statement: "成果を測定した", reviewState: "accepted" }, { ...baseClaim, id: "old", statement: "編集前の主張", reviewState: "edited" }] });
    vi.mocked(api.listEvidenceRelations).mockResolvedValue([{ id: "r", fromClaimId: "a", toClaimId: "b", relationType: "supports", createdBy: "user", createdAt: "now" }]);
    vi.mocked(api.createEvidenceRelation).mockResolvedValue({ id: "new", fromClaimId: "a", toClaimId: "b", relationType: "supports", createdBy: "user", createdAt: "now" }); vi.mocked(api.deleteEvidenceRelation).mockResolvedValue();
    renderPage(); const from = await screen.findByLabelText("起点の根拠"); expect(from).not.toHaveTextContent("編集前の主張"); await user.selectOptions(from, "a"); await user.selectOptions(screen.getByLabelText("相手の根拠"), "b"); await user.click(screen.getByRole("button", { name: "関係を作成" }));
    expect(api.createEvidenceRelation).toHaveBeenCalledWith({ fromClaimId: "a", toClaimId: "b", relationType: "supports" }); expect(screen.queryByText("不明な主張")).not.toBeInTheDocument(); await user.click(screen.getByRole("button", { name: "関係を解除" })); expect(api.deleteEvidenceRelation).toHaveBeenCalledWith("r");
  });
  it("reopens an excluded claim", async () => {
    const user = userEvent.setup(); vi.mocked(api.listEvidenceLibrary).mockResolvedValue({ counts: { sourceCount: 0, pendingClaimCount: 0, acceptedClaimCount: 0, inferenceCount: 0, projectCount: 0, privateDraftCount: 0 }, sources: [], claims: [{ ...baseClaim, id: "excluded", statement: "保留した主張", reviewState: "excluded", portfolioEligible: false }] }); vi.mocked(api.reviewEvidenceClaim).mockResolvedValue({ ...baseClaim, id: "excluded", statement: "保留した主張", reviewState: "pending", portfolioEligible: false }); renderPage(); await user.click(await screen.findByRole("button", { name: "確認待ちへ戻す" })); expect(api.reviewEvidenceClaim).toHaveBeenCalledWith({ claimId: "excluded", decision: "reopen", editedStatement: null, portfolioEligible: false });
  });
  it("also reopens a deferred claim", async () => {
    const user = userEvent.setup(); vi.mocked(api.listEvidenceLibrary).mockResolvedValue({ counts: { sourceCount: 0, pendingClaimCount: 0, acceptedClaimCount: 0, inferenceCount: 0, projectCount: 0, privateDraftCount: 0 }, sources: [], claims: [{ ...baseClaim, id: "deferred", statement: "後で確認する主張", reviewState: "deferred", portfolioEligible: false }] }); vi.mocked(api.reviewEvidenceClaim).mockResolvedValue({ ...baseClaim, id: "deferred", statement: "後で確認する主張", reviewState: "pending", portfolioEligible: false }); renderPage(); await user.click(await screen.findByRole("button", { name: "確認待ちへ戻す" })); expect(api.reviewEvidenceClaim).toHaveBeenCalledWith({ claimId: "deferred", decision: "reopen", editedStatement: null, portfolioEligible: false });
  });
  it("resets payload and redaction acknowledgement for each newly imported source", async () => {
    const user = userEvent.setup(); const imported = (id: string) => ({ imported: [{ document: { id, contentSha256: id, contentText: "secret", byteLength: 6, lineCount: 1, createdAt: "now" }, occurrence: { id: `occ-${id}`, sourceDocumentId: id, sourceKind: "paste" as const, displayName: id, originalPath: null, importedAt: "now" }, duplicateContent: false }], failures: [] });
    vi.mocked(api.importPastedSource).mockResolvedValueOnce(imported("source-1")).mockResolvedValueOnce(imported("source-2")); vi.mocked(api.getEvidenceAnalysisPreview).mockImplementation(async (id) => id === "source-1" ? { sourceId: id, submittedPayload: "first payload", cloudInferenceNotice: "cloud", redactionFindings: [{ kind: "token", startByte: 0, endByte: 6 }], needsReview: true } : { sourceId: id, submittedPayload: "second payload", cloudInferenceNotice: "cloud", redactionFindings: [], needsReview: false });
    renderImport(); await user.type(screen.getByLabelText("記録名"), "one"); await user.type(screen.getByLabelText("原文"), "secret"); await user.click(screen.getByRole("button", { name: "原文を保存" })); const editor = await screen.findByLabelText("送信するJSON"); await user.clear(editor); await user.type(editor, "edited payload"); await user.click(screen.getByRole("checkbox"));
    await user.clear(screen.getByLabelText("記録名")); await user.type(screen.getByLabelText("記録名"), "two"); await user.click(screen.getByRole("button", { name: "原文を保存" })); expect(await screen.findByLabelText("送信するJSON")).toHaveValue("second payload"); expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  });
});
