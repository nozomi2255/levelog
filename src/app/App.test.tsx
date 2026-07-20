import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AppProviders } from "./providers";
import { App } from "./App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((command: string) => {
    if (command === "get_boot_state") return Promise.resolve({ onboardingComplete: true, codex: null });
    if (command === "get_dashboard") return Promise.resolve({
      level: 1,
      totalXp: 0,
      xpToNextLevel: 100,
      todayXp: 0,
      todayActivities: 0,
      todayObservations: 0,
      activeQuest: null,
      recentActivities: [],
      weeklyXp: [],
      categoryObservations: [],
    });
    return Promise.resolve(null);
  }),
}));

function renderApp() {
  return render(<AppProviders><App /></AppProviders>);
}

describe("App", () => {
  afterEach(cleanup);

  it("shows the seven primary navigation routes", async () => {
    renderApp();
    expect(await screen.findByRole("navigation", { name: "メインナビゲーション" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /^ホーム/i })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("link", { name: /クエスト/i })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /設定/i })).toBeInTheDocument();
  });

  it("keeps the dashboard explicit when no data has been loaded", async () => {
    renderApp();
    expect(await screen.findByRole("heading", { name: "最初の活動を記録しましょう" })).toBeInTheDocument();
    expect(screen.getByText("まだ承認済みのアクティビティはありません。")).toBeInTheDocument();
    expect(screen.getByText("活動を承認すると、週間推移が表示されます。")).toBeInTheDocument();
  });
});
