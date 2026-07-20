import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../../lib/api";
import type { QuestDto, QuestStatus } from "../../lib/types";
import { QuestsPage } from "./QuestsPage";

vi.mock("../../lib/api", () => ({ api: { listQuests: vi.fn(), transitionQuest: vi.fn(), saveQuestReflection: vi.fn() } }));

const quest = (status: QuestStatus): QuestDto => ({
  id: `quest-${status}`, templateId: "clarify_once", title: `${status}のクエスト`, description: "一つ確認する",
  targetSkillId: "communication.clarification", estimatedMinutes: 10, difficulty: 2,
  successCriteria: ["確認を一つ送る"], evidencePrompt: "返答を記録", status, scheduledOn: null,
});

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return render(<QueryClientProvider client={client}><MemoryRouter><QuestsPage /></MemoryRouter></QueryClientProvider>);
}

describe("QuestsPage", () => {
  beforeEach(() => vi.mocked(api.listQuests).mockResolvedValue(["proposed", "accepted", "in_progress", "completed", "rescheduled", "adjusted", "cancelled"].map((status) => quest(status as QuestStatus))));

  it("exposes every MVP state action and the outcome-neutral reflection form", async () => {
    const user = userEvent.setup();
    renderPage();
    expect(await screen.findByRole("heading", { name: "クエスト", level: 1 })).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "受注する" }).length).toBeGreaterThan(0);
    expect(screen.getAllByRole("button", { name: "開始する" }).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "完了にする" })).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "延期する" }).length).toBeGreaterThan(0);
    expect(screen.getAllByRole("button", { name: "縮小する" }).length).toBeGreaterThan(0);
    expect(screen.getAllByRole("button", { name: "破棄する" }).length).toBeGreaterThan(0);
    await user.click(screen.getAllByRole("button", { name: "破棄する" })[0]!);
    expect(screen.getByText("このクエストは履歴に残りますが、破棄後は再開できません。")).toBeInTheDocument();
    expect(api.transitionQuest).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "本当に破棄する" }));
    expect(api.transitionQuest).toHaveBeenCalledWith(expect.objectContaining({ action: "cancel" }), expect.anything());
    await user.click(screen.getByRole("button", { name: "振り返りを記録する" }));
    expect(screen.getByRole("option", { name: "未完了" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "休息した" })).toBeInTheDocument();
  });
});
