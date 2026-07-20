import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const claim = { id: "claim-1", sourceDocumentId: "source-1", sourceOccurrenceId: "occ-1", supersedesClaimId: null, kind: "experience", provenance: "import_extracted", statement: "APIを改善した", sourceExcerpt: "APIを改善した", startByte: null, endByte: null, confidence: .8, reviewState: "pending", portfolioEligible: false, linkedSkillIds: [], createdAt: "now", reviewedAt: null };
    let claims = [claim]; const projects: Array<Record<string, unknown>> = []; const drafts: Array<Record<string, unknown>> = [];
    const invoke = async (command: string, args?: { input: Record<string, unknown> }) => {
      if (command === "get_boot_state") return { onboardingComplete: true, codex: null };
      if (command === "get_user_profile") return null;
      if (command === "list_evidence_library") { const visibleClaims = args?.input.reviewState ? claims.filter((c) => c.reviewState === args.input.reviewState) : claims; return { sources: [{ id: "occ-1", sourceDocumentId: "source-1", sourceKind: "paste", displayName: "note", originalPath: null, importedAt: "2026-07-20T00:00:00Z" }], claims: visibleClaims, counts: { sourceCount: 1, pendingClaimCount: claims.filter(c => c.reviewState === "pending").length, acceptedClaimCount: claims.filter(c => c.reviewState === "accepted").length, inferenceCount: 0, projectCount: projects.length, privateDraftCount: drafts.length } }; }
      if (command === "import_pasted_source") return { imported: [{ document: { id: "source-1", contentSha256: "x", contentText: String(args?.input.contentText ?? ""), byteLength: 1, lineCount: 1, createdAt: "now" }, occurrence: { id: "occ-1", sourceDocumentId: "source-1", sourceKind: "paste", displayName: String(args?.input.displayName ?? ""), originalPath: null, importedAt: "now" }, duplicateContent: false }], failures: [] };
      if (command === "get_evidence_analysis_preview") return { sourceId: "source-1", submittedPayload: "{}", cloudInferenceNotice: "送信内容です", redactionFindings: [], needsReview: false };
      if (command === "list_evidence_relations") return [];
      if (command === "get_evidence_source") return { document: { id: "source-1", contentSha256: "x", contentText: "APIを改善した", byteLength: 1, lineCount: 1, createdAt: "now" }, occurrences: [{ id: "occ-1", sourceDocumentId: "source-1", sourceKind: "paste", displayName: "note", originalPath: null, importedAt: "now" }], claims };
      if (command === "review_evidence_claim") { claims = claims.map(c => c.id === args?.input.claimId ? { ...c, reviewState: "accepted", portfolioEligible: true } : c); return claims[0]; }
      if (command === "list_projects") return projects;
      if (command === "create_project") { const p = { id: "project-1", ...args?.input, evidenceCount: 0, createdAt: "now", updatedAt: "now" }; projects.push(p); return p; }
      if (command === "get_project") return { ...projects[0], claims: [] };
      if (command === "link_claim_to_project") return { ...projects[0], claims };
      if (command === "unlink_claim_from_project") return { ...projects[0], claims: [] };
      if (command === "list_portfolio_drafts") return drafts;
      if (command === "create_portfolio_draft") { const d = { id: "draft-1", ...args?.input, bodyMarkdown: "", privacyState: "private", createdAt: "now", updatedAt: "now" }; drafts.push(d); return d; }
      return null;
    };
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: { invoke }, configurable: true });
  });
});

test("paste, review, project link, provenance, and private draft remain keyboard-accessible", async ({ page }) => {
  await page.goto("/memory/import");
  await page.getByLabel("記録名").fill("note"); await page.getByLabel("原文").fill("APIを改善した"); await page.getByRole("button", { name: "原文を保存" }).click();
  await expect(page.getByText("原文をローカルに保存しました。")).toBeVisible();
  await page.getByRole("link", { name: "エビデンス", exact: true }).click(); await page.getByRole("link", { name: "候補を確認" }).click(); await page.getByRole("button", { name: "採用" }).click(); await expect(page.getByText("確認待ちの候補はありません。")).toBeVisible();
  await page.getByRole("link", { name: "ライブラリへ" }).click(); await page.getByRole("link", { name: "プロジェクト" }).click(); await page.getByLabel("名前").fill("Levelog"); await page.getByLabel("概要").fill("個人開発"); await page.getByRole("button", { name: "作成" }).click(); await page.getByRole("link", { name: /Levelog active/ }).click(); await page.getByRole("button", { name: "このプロジェクトへリンク" }).click();
  await page.getByRole("link", { name: "原文と出所を見る" }).click(); await expect(page.getByText("APIを改善した").first()).toBeVisible();
  await page.getByRole("link", { name: "ライブラリへ" }).click(); await page.getByRole("link", { name: "ポートフォリオ" }).click(); await page.getByLabel("タイトル").fill("私の実績"); await page.getByLabel("用途").fill("紹介"); await page.getByRole("checkbox").check(); await page.getByRole("button", { name: "非公開下書きを作成" }).click(); await expect(page.getByText("非公開", { exact: true })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBeTruthy();
});
