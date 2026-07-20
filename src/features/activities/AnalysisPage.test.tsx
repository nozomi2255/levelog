import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../../lib/api";
import type { ActivityAnalysisDto, ActivityDetailDto } from "../../lib/types";
import { AnalysisPage } from "./AnalysisPage";

vi.mock("../../lib/api", () => ({
  api: {
    getActivity: vi.fn(), getAnalysisPreview: vi.fn(), getActivityAnalysis: vi.fn(),
    startActivityAnalysis: vi.fn(), cancelActivityAnalysis: vi.fn(),
    confirmActivityAnalysis: vi.fn(), generateQuest: vi.fn(),
  },
}));

const analysis: ActivityAnalysisDto = {
  id: "analysis-1", activityId: "activity-1", status: "succeeded", summary: "要件を整理した",
  outcomes: ["認識を揃えた"], missingInformationQuestion: "相手の反応は？", errorMessage: null,
  skillCandidates: [{ id: "candidate-1", skillId: "thinking.information_structuring", confidence: .8, reason: "情報を分けた", evidence: "三つの確認事項にした", decision: "pending" }],
};
const activity: ActivityDetailDto = {
  id: "activity-1", occurredOn: "2026-07-20", actionText: "要件を整理した", challengeText: "", outcomeText: "認識が揃った", createdAt: "2026-07-20T00:00:00.000Z", analysisStatus: null, analyses: [],
};

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return render(<QueryClientProvider client={client}><MemoryRouter initialEntries={["/activities/activity-1/analysis"]}><Routes><Route path="/activities/:activityId/analysis" element={<AnalysisPage />} /></Routes></MemoryRouter></QueryClientProvider>);
}

describe("AnalysisPage", () => {
  beforeEach(() => {
    vi.mocked(api.getActivity).mockResolvedValue(activity);
    vi.mocked(api.getAnalysisPreview).mockResolvedValue({ activityId: "activity-1", submittedPayload: "{\n  \"activity\": \"safe\"\n}", cloudInferenceNotice: "Codexへ送信されます" });
    vi.mocked(api.getActivityAnalysis).mockResolvedValue(analysis);
    vi.mocked(api.confirmActivityAnalysis).mockResolvedValue({ analysisId: "analysis-1", confirmedObservationCount: 1, xpAwarded: 20 });
  });

  it("shows an editable payload before an analysis starts", async () => {
    renderPage();
    expect(await screen.findByRole("heading", { name: "AI分析の確認" })).toBeInTheDocument();
    expect((screen.getByLabelText("送信するJSON") as HTMLTextAreaElement).value).toContain("safe");
    expect(screen.getByText(/Codexへ送信されます/)).toBeInTheDocument();
  });

  it.each([
    ["running", "分析をキャンセル"],
    ["failed", "送信内容に戻って再試行"],
  ] as const)("renders the %s recovery state", async (status, buttonName) => {
    vi.mocked(api.getActivity).mockResolvedValue({ ...activity, analyses: [{ ...analysis, status }] });
    vi.mocked(api.getActivityAnalysis).mockResolvedValue({ ...analysis, status, errorMessage: status === "failed" ? "非JSON出力" : null });
    renderPage();
    expect(await screen.findByRole("button", { name: buttonName })).toBeInTheDocument();
  });

  it("defaults pending AI candidates to rejection and submits an explicit user decision", async () => {
    const user = userEvent.setup();
    vi.mocked(api.getActivity).mockResolvedValue({ ...activity, analysisStatus: "succeeded", analyses: [analysis] });
    renderPage();
    const rejected = await screen.findByRole("radio", { name: "却下" });
    expect(rejected).toBeChecked();
    await user.click(screen.getByRole("radio", { name: "採用" }));
    await user.click(screen.getByRole("button", { name: "判断を確定する（+20 XP）" }));
    await waitFor(() => expect(api.confirmActivityAnalysis).toHaveBeenCalledWith({ analysisId: "analysis-1", candidateDecisions: [{ candidateId: "candidate-1", decision: "accepted", editedReason: null, editedEvidence: null }] }));
  });
});
