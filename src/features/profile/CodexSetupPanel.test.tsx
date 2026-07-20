import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { api } from "../../lib/api";
import { CodexSetupPanel } from "./CodexSetupPanel";

vi.mock("../../lib/api", () => ({ api: { discoverCodexCandidates: vi.fn(), testCodexConnection: vi.fn(), updateCodexPath: vi.fn() } }));

describe("CodexSetupPanel", () => {
  afterEach(() => { cleanup(); vi.clearAllMocks(); });
  it("only saves a selected candidate after a READY probe", async () => {
    const user = userEvent.setup(); vi.mocked(api.discoverCodexCandidates).mockResolvedValue([{ discoveredPath: "/opt/homebrew/bin/codex", canonicalPath: "/opt/homebrew/bin/codex", source: "homebrew", executable: true, recommended: true, connection: null }]); vi.mocked(api.testCodexConnection).mockResolvedValue({ available: true, authenticated: true, path: "/opt/homebrew/bin/codex", version: "codex 1", message: "接続できました" }); vi.mocked(api.updateCodexPath).mockResolvedValue({ role: "dev", focusSkillIds: ["communication.explanation"], weeklyMinutes: 120, excludedQuestPatterns: "", codexPath: "/opt/homebrew/bin/codex", onboardingComplete: true, updatedAt: "now" });
    render(<CodexSetupPanel />);
    expect(screen.getByRole("button", { name: "READYの接続を保存" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Codexを自動検出" }));
    expect(await screen.findByLabelText(/\/opt\/homebrew\/bin\/codex/)).toBeChecked();
    await user.click(screen.getByRole("button", { name: "選択した候補を接続テスト" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "READYの接続を保存" })).toBeEnabled());
    await user.click(screen.getByRole("button", { name: "READYの接続を保存" }));
    await waitFor(() => expect(api.updateCodexPath).toHaveBeenCalledWith("/opt/homebrew/bin/codex"));
  });
  it("does not retain a hidden manual path when automatic selection is restored", async () => {
    const user = userEvent.setup(); vi.mocked(api.discoverCodexCandidates).mockResolvedValue([{ discoveredPath: "/opt/homebrew/bin/codex", canonicalPath: "/opt/homebrew/bin/codex", source: "homebrew", executable: true, recommended: true, connection: null }]); vi.mocked(api.testCodexConnection).mockResolvedValue({ available: true, authenticated: true, path: "/opt/homebrew/bin/codex", version: "codex 1", message: "接続できました" });
    render(<CodexSetupPanel />);
    await user.click(screen.getByRole("button", { name: "Codexを自動検出" }));
    const manual = screen.getByLabelText("別の場所を指定する");
    await user.click(manual);
    await user.clear(screen.getByLabelText("Codex CLIの絶対パス"));
    await user.type(screen.getByLabelText("Codex CLIの絶対パス"), "/custom/codex");
    await user.click(manual);
    await user.click(screen.getByRole("button", { name: "選択した候補を接続テスト" }));
    await waitFor(() => expect(api.testCodexConnection).toHaveBeenCalledWith("/opt/homebrew/bin/codex"));
  });
});
