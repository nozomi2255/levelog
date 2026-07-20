import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const now = "2026-07-20T06:00:00.000Z";
    let onboardingComplete = !window.location.search.includes("onboarding=1");
    const activities: Array<Record<string, unknown>> = [];
    const dashboard = {
      level: 1, totalXp: 0, xpToNextLevel: 100, todayXp: 0, todayActivities: 0,
      todayObservations: 0, activeQuest: null, recentActivities: [], weeklyXp: [], categoryObservations: [],
    };
    const invoke = async (command: string, args?: Record<string, unknown>) => {
      if (command === "get_boot_state") return { onboardingComplete, codex: onboardingComplete ? { available: false, authenticated: false, path: "/usr/local/bin/codex", version: null, message: "接続テスト未実行" } : null };
      if (command === "save_onboarding") { onboardingComplete = true; return { ...(args?.input as object), onboardingComplete: true, updatedAt: now }; }
      if (command === "get_dashboard") return dashboard;
      if (command === "list_activities") return activities;
      if (command === "list_quests" || command === "list_skills") return [];
      if (command === "create_activity") {
        const activity = { id: "activity-1", ...(args?.input as object), createdAt: now, analysisStatus: null };
        activities.push(activity); return activity;
      }
      if (command === "get_activity") return { ...(activities[0] ?? { id: "activity-1", occurredOn: "2026-07-20", actionText: "", challengeText: "", outcomeText: "", createdAt: now, analysisStatus: null }), analyses: [] };
      if (command === "get_analysis_preview") return { activityId: "activity-1", submittedPayload: JSON.stringify({ activity: activities[0] }, null, 2), cloudInferenceNotice: "この内容はCodexの推論先へ送信されます。" };
      if (command === "test_codex_connection") return { available: true, authenticated: true, path: "/usr/local/bin/codex", version: "codex 1.0", message: "接続できました" };
      return null;
    };
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: { invoke }, configurable: true });
  });
});

test("shell and empty-state routes render at each configured viewport", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("navigation", { name: "メインナビゲーション" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "最初の活動を記録しましょう" })).toBeVisible();
  for (const [label, heading] of [["クエスト", "クエスト"], ["アクティビティ", "アクティビティ"], ["スキル", "スキル観測"], ["設定", "設定"]] as const) {
    await page.getByRole("link", { name: label, exact: true }).click();
    await expect(page.getByRole("heading", { name: heading, exact: true, level: 1 })).toBeVisible();
  }
});

test("onboarding and activity capture expose the minimum safe flow", async ({ page }) => {
  await page.goto("/?onboarding=1");
  await expect(page.getByRole("heading", { name: "成長プロフィールを設定" })).toBeVisible();
  await page.getByLabel("現在の役割").fill("エンジニア");
  await page.getByLabel("情報整理").check();
  await page.getByLabel("Codex CLIの絶対パス").fill("/usr/local/bin/codex");
  await page.getByRole("button", { name: "保存してLevelogを開始" }).click();
  await expect(page.getByRole("heading", { name: "最初の活動を記録しましょう" })).toBeVisible();
  await page.goto("/activities/new");
  await page.getByLabel("何をした？").fill("要件を三つの確認事項に分けた");
  await page.getByRole("button", { name: "保存して送信内容を確認" }).click();
  await expect(page.getByRole("heading", { name: "AI分析の確認" })).toBeVisible();
  await expect(page.getByLabel("送信するJSON")).toContainText("要件を三つの確認事項に分けた");
});

test("keyboard focus remains visible in shell navigation", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("navigation", { name: "メインナビゲーション" })).toBeVisible();
  await page.keyboard.press("Tab");
  await expect(page.locator(":focus")).toHaveClass(/brand/);
  await page.keyboard.press("Tab");
  await expect(page.locator(":focus")).toHaveClass(/nav-item/);
});

test("reduced-motion users do not receive an animated growth core", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/");
  const duration = await page.locator(".growth-core__rings").evaluate((element) => getComputedStyle(element).animationDuration);
  expect(Number.parseFloat(duration)).toBeLessThanOrEqual(0.00001);
});

test("dashboard matches the approved responsive visual baseline", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "最初の活動を記録しましょう" })).toBeVisible();
  await expect(page).toHaveScreenshot("dashboard-empty.png", { fullPage: true, animations: "disabled" });
});
