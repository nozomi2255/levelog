import { expect, test } from "@playwright/test";

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const now = "2026-07-20T06:00:00.000Z";
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    let questionAnswered = false;
    let questionDeferred = false;
    let onboardingComplete = !window.location.search.includes("onboarding=1");
    const activities: Array<Record<string, unknown>> = [];
    const question = { sessionId: "session-1", questionId: "question-1", target: "outcome", text: "この判断で何が変わりましたか？", answerType: "single_choice", choices: [{ value: "alignment", label: "認識がそろった" }, { value: "speed", label: "作業が速くなった" }], whyItMatters: "結果を推測ではなく事実として扱うためです。", status: "pending" };
    const candidates = [
      { id: "candidate-1", skillId: "thinking.problem_decomposition", confidence: 0.9, reason: "課題を分けた", evidence: "要件を三つに分解", decision: "pending", specializedSkillName: "要件分解" },
      { id: "candidate-2", skillId: "technical.system_design", confidence: 0.7, reason: "設計を見直した", evidence: "レビューで検討", decision: "pending", specializedSkillName: null },
      { id: "candidate-3", skillId: "technical.validation", confidence: 0.6, reason: "確認した", evidence: "検証した", decision: "pending", specializedSkillName: null },
    ];
    const profile = {
      schemaVersion: 2, revision: 1, role: "プロダクトエンジニア", background: "B2B SaaSの開発経験",
      currentResponsibilities: "設計と顧客との認識合わせ", domainsAndTechnologies: ["TypeScript", "プロダクト設計"],
      growthGoal: "仮説を短く検証する", motivation: "顧客価値を早く届けるため", currentChallenges: "優先順位の合意",
      recentSuccess: "要件を三つに分解した", focusSkillIds: ["thinking.problem_decomposition", "technical.system_design"], weeklyMinutes: 120,
      preferredQuestMinutes: 30, preferredQuestStyle: "work_integrated", constraints: "平日30分まで", excludedQuestPatterns: "長時間の座学",
      focusThemes: [{ id: "theme-1", title: "発見から検証までを速くする", desiredOutcome: "仮説検証を週に一度行う", whyNow: "今期の重点", horizon: "quarter", status: "active", linkedSkillIds: ["thinking.problem_decomposition"], sortOrder: 0, updatedAt: now }],
      updatedAt: now,
    };
    const dashboard = {
      level: 1, totalXp: 0, xpToNextLevel: 100, todayXp: 0, todayActivities: 0,
      todayObservations: 0, activeQuest: null, recentActivities: [], weeklyXp: [], categoryObservations: [],
    };
    const invoke = async (command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args });
      if (command === "get_boot_state") return { onboardingComplete, codex: onboardingComplete ? { available: false, authenticated: false, path: "/usr/local/bin/codex", version: null, message: "接続テスト未実行" } : null };
      if (command === "save_onboarding") { onboardingComplete = true; return { ...(args?.input as object), onboardingComplete: true, updatedAt: now }; }
      // Keep the approved empty-dashboard visual fixture profile-free. Route tests
      // that exercise profile-aware surfaces receive the same deterministic record.
      if (command === "get_user_profile") return window.location.pathname === "/" ? null : profile;
      if (command === "update_user_profile") { Object.assign(profile, args?.input ?? {}, { revision: profile.revision + 1, updatedAt: now }); onboardingComplete = true; return profile; }
      if (command === "list_focus_themes") return profile.focusThemes;
      if (command === "save_focus_themes") { profile.focusThemes = ((args?.input as { themes?: unknown[] })?.themes ?? []) as typeof profile.focusThemes; return profile.focusThemes; }
      if (command === "discover_codex_candidates") return [{ discoveredPath: "/Applications/Codex.app/Contents/Resources/codex", canonicalPath: "/Applications/Codex.app/Contents/Resources/codex", source: "Codex.app", executable: true, recommended: true, connection: null }];
      if (command === "get_release_info") return { currentVersion: "0.1.0", updaterConfigured: false, releaseChannel: "GitHub Releases / stable" };
      if (command === "get_dashboard") return dashboard;
      if (command === "list_activities") return activities;
      if (command === "list_activity_inbox") return activities.map((activity) => ({ ...activity, workflow: { activityId: activity.id, state: "captured", version: 1, currentQuestion: null, updatedAt: now } }));
      if (command === "list_quests" || command === "list_skills") return [];
      if (command === "create_activity") {
        const activity = { id: "activity-1", ...(args?.input as object), createdAt: now, analysisStatus: null };
        activities.push(activity); return activity;
      }
      if (command === "quick_capture_activity") {
        const input = (args?.input ?? {}) as { occurredOn?: string; rawText?: string };
        const activity = { id: "activity-1", occurredOn: input.occurredOn ?? "2026-07-20", actionText: input.rawText ?? "", challengeText: "", outcomeText: "", createdAt: now, analysisStatus: null };
        activities.push(activity); return activity;
      }
      if (command === "get_activity") {
        const id = args?.activityId as string;
        const status = id === "quest-1" ? "confirmed" : "succeeded";
        return { ...(activities.find((activity) => activity.id === id) ?? { id, occurredOn: "2026-07-20", actionText: "要件を三つに分解した", challengeText: "", outcomeText: "", createdAt: now, analysisStatus: status }), analyses: [{ id: `${id}-analysis`, activityId: id, status }] };
      }
      if (command === "get_analysis_preview") return { activityId: "activity-1", submittedPayload: JSON.stringify({ activity: activities[0] }, null, 2), cloudInferenceNotice: "この内容はCodexの推論先へ送信されます。" };
      if (command === "get_activity_workflow") {
        const id = args?.activityId as string;
        const questionScenario = id === "question-1";
        const state = questionScenario ? (!questionAnswered || questionDeferred ? "needs_input" : "assessable") : id === "quest-1" ? "confirmed" : "review_pending";
        return { activityId: id, state, version: questionAnswered ? 2 : 1, currentQuestion: questionScenario && (!questionAnswered || questionDeferred) ? question : null, updatedAt: now };
      }
      if (command === "get_activity_analysis") {
        const id = args?.analysisId as string;
        const activityId = id.replace(/-analysis$/, "");
        const status = activityId === "quest-1" ? "confirmed" : "succeeded";
        return { id, activityId, status, summary: "経験を整理しました", outcomes: ["認識合わせが進んだ"], confirmedFacts: ["要件を三つに分解した"], unconfirmedFacts: ["最終的な影響"], skillCandidates: candidates, missingInformationQuestion: null, nextQuestion: activityId === "question-1" ? question : null, errorMessage: null };
      }
      if (command === "answer_activity_question") { questionAnswered = true; questionDeferred = ((args?.input as { answerState?: string })?.answerState === "deferred"); return { activityId: "question-1", state: questionDeferred ? "needs_input" : "assessable", version: 2, currentQuestion: questionDeferred ? question : null, updatedAt: now }; }
      if (command === "confirm_activity_analysis") return { analysisId: (args?.input as { analysisId?: string })?.analysisId, confirmedObservationCount: 2, xpAwarded: 20 };
      if (command === "get_quest_preview") return { entityId: "quest-1", submittedPayload: JSON.stringify({ purpose: "next quest" }, null, 2), cloudInferenceNotice: "この内容はCodexの推論先へ送信されます。" };
      if (command === "generate_quest") return { id: "quest-generated", templateId: "weekly", title: "次の一歩", description: "小さく検証する", targetSkillId: "technical.validation", estimatedMinutes: 20, difficulty: 2, successCriteria: ["一度試す"], evidencePrompt: "記録する", status: "proposed", scheduledOn: null };
      if (command === "test_codex_connection") return { available: true, authenticated: true, path: "/usr/local/bin/codex", version: "codex 1.0", message: "接続できました" };
      if (command === "update_codex_path") return { onboardingComplete: true, updatedAt: now };
      return null;
    };
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: { invoke }, configurable: true });
    Object.defineProperty(window, "__levelogTest", { value: { calls }, configurable: true });
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

test("onboarding can defer Codex and quick capture reaches the local inbox", async ({ page }) => {
  await page.goto("/?onboarding=1");
  await expect(page.getByRole("heading", { name: "成長プロフィールを設定" })).toBeVisible();
  await page.getByLabel("現在の役割").fill("エンジニア");
  await page.getByLabel("成長目標").fill("仮説検証を短く回す");
  await page.getByLabel("情報整理").check();
  await page.getByRole("button", { name: "プロフィールを保存してAI設定へ" }).click();
  await expect(page.getByRole("heading", { name: "Codex CLIを接続" })).toBeVisible();
  await page.getByRole("button", { name: "AI接続は後で" }).click();
  await expect(page.getByRole("heading", { name: "最初の活動を記録しましょう" })).toBeVisible();
  await page.goto("/activities/new");
  await page.getByLabel("起きたこと").fill("要件を三つの確認事項に分けた");
  await page.getByRole("button", { name: "一言を保存する（+10 XP）" }).click();
  await expect(page.getByRole("heading", { name: "整理インボックス" })).toBeVisible();
  await expect(page.getByText("要件を三つの確認事項に分けた")).toBeVisible();
});

test("onboarding can save an auto-detected READY Codex connection", async ({ page }) => {
  await page.goto("/?onboarding=1");
  await page.getByLabel("現在の役割").fill("エンジニア");
  await page.getByLabel("成長目標").fill("仮説検証を短く回す");
  await page.getByLabel("情報整理").check();
  await page.getByRole("button", { name: "プロフィールを保存してAI設定へ" }).click();
  await page.getByRole("button", { name: "Codexを自動検出" }).click();
  await page.getByRole("button", { name: "選択した候補を接続テスト" }).click();
  await expect(page.getByText("READY", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "READYの接続を保存" }).click();
  await expect(page.getByRole("heading", { name: "最初の活動を記録しましょう" })).toBeVisible();
  const savedPaths = await page.evaluate(() => (window as unknown as { __levelogTest: { calls: Array<{ command: string; args: { input: { codexPath: string } } }> } }).__levelogTest.calls.filter((call) => call.command === "update_codex_path").map((call) => call.args.input.codexPath));
  expect(savedPaths).toEqual(["/usr/local/bin/codex"]);
});

test("one structured question supports answer, unknown, skip, and defer choices", async ({ page }) => {
  for (const [button, answerState] of [["回答を保存", "answered"], ["わからない", "unknown"], ["スキップ", "skipped"], ["後で答える", "deferred"]] as const) {
    await page.goto("/activities/question-1/analysis");
    await expect(page.getByRole("heading", { name: "経験を整理する" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "この判断で何が変わりましたか？", exact: true, level: 2 })).toBeVisible();
    if (answerState === "answered") await page.getByRole("radio", { name: "認識がそろった" }).check();
    await expect(page.getByRole("button", { name: button })).toBeEnabled();
    await page.getByRole("button", { name: button }).click();
    if (answerState === "deferred") await expect(page.getByText("質問を後で答える項目として保存しました。整理インボックスから再開できます。")).toBeVisible();
    else await expect(page.getByRole("button", { name: "回答を反映して再解析" })).toBeVisible();
    const answerStates = await page.evaluate(() => (window as unknown as { __levelogTest: { calls: Array<{ command: string; args: { input: { answerState: string } } }> } }).__levelogTest.calls.filter((call) => call.command === "answer_activity_question").map((call) => call.args.input.answerState));
    expect(answerStates).toEqual([answerState]);
  }
});

test("analysis requires a decision for every candidate and exposes the exact quest preview", async ({ page }) => {
  await page.goto("/activities/decision-1/analysis");
  await expect(page.getByRole("heading", { name: "分析候補" })).toBeVisible();
  const cards = page.locator(".candidate-card");
  await expect(cards).toHaveCount(3);
  await cards.nth(0).getByLabel("採用", { exact: true }).check();
  await cards.nth(1).getByLabel("編集して採用", { exact: true }).check();
  await cards.nth(1).getByLabel("専門スキル名（任意）").fill("設計レビュー");
  await cards.nth(2).getByLabel("却下", { exact: true }).check();
  await expect(page.getByRole("button", { name: "判断を確定する（+20 XP）" })).toBeEnabled();
  await page.getByRole("button", { name: "判断を確定する（+20 XP）" }).click();
  await expect(page.getByText("2件の証拠を確定し、20 XPを追加しました。")).toBeVisible();

  await page.goto("/activities/quest-1/analysis");
  await expect(page.getByLabel("クエストに送信するJSON")).toContainText("next quest");
  const editedQuestPayload = JSON.stringify({ purpose: "edited next quest", preferredMinutes: 15 });
  await page.getByLabel("クエストに送信するJSON").fill(editedQuestPayload);
  await page.getByRole("button", { name: "内容を確認して提案を生成" }).click();
  await expect(page.getByText("クエスト「次の一歩」を作成しました。")).toBeVisible();
  const submitted = await page.evaluate(() => (window as unknown as { __levelogTest: { calls: Array<{ command: string; args: { input: { submittedPayload: string } } }> } }).__levelogTest.calls.filter((call) => call.command === "generate_quest").map((call) => call.args.input.submittedPayload));
  expect(submitted).toEqual([editedQuestPayload]);
});

test("keyboard focus remains visible in shell navigation", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByRole("navigation", { name: "メインナビゲーション" })).toBeVisible();
  await page.keyboard.press("Tab");
  await expect(page.locator(":focus")).toHaveClass(/skip-link/);
  await page.keyboard.press("Tab");
  await expect(page.locator(":focus")).toHaveClass(/brand/);
  await page.keyboard.press("Tab");
  await expect(page.locator(":focus")).toHaveClass(/sidebar-capture/);
  await page.keyboard.press("Tab");
  await expect(page.locator(":focus")).toHaveClass(/nav-item/);
});

test("settings keeps profile, personal themes, and safe Codex discovery operable by keyboard", async ({ page }) => {
  await page.goto("/settings");
  await expect(page.getByRole("heading", { name: "成長プロフィール", exact: true })).toBeVisible();
  await expect(page.getByRole("form", { name: "成長プロフィールを編集" })).toBeVisible();
  await expect(page.getByLabel("現在の役割")).toHaveValue("プロダクトエンジニア");
  await page.getByLabel("現在の役割").fill("プロダクトリード");
  await page.getByRole("button", { name: "プロフィールを保存" }).click();
  await expect(page.getByText("成長プロフィールを保存しました。")).toBeVisible();

  await page.getByRole("button", { name: "Codexを自動検出" }).focus();
  await expect(page.locator(":focus")).toHaveAccessibleName("Codexを自動検出");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("group", { name: "検出された候補" })).toBeVisible();
  await expect(page.getByText("/Applications/Codex.app/Contents/Resources/codex")).toBeVisible();
  const detectButton = page.getByRole("button", { name: "Codexを自動検出" });
  await expect(detectButton).toHaveCSS("min-height", /44px|2\.75rem/);
});

test("responsive shell has no horizontal overflow and its primary controls remain reachable", async ({ page }) => {
  await page.goto("/settings");
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBeTruthy();
  await expect(page.getByRole("link", { name: "経験を記録" }).first()).toBeVisible();
  await page.getByRole("link", { name: "スキル", exact: true }).click();
  await expect(page.getByRole("heading", { name: "スキル観測", exact: true })).toBeVisible();
  await expect(page.getByText("発見から検証までを速くする")).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBeTruthy();
});

test("reduced-motion users do not receive an animated growth core", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/");
  const duration = await page.locator(".growth-core__rings").evaluate((element) => getComputedStyle(element).animationDuration);
  expect(Number.parseFloat(duration)).toBeLessThanOrEqual(0.00001);
});

test("dashboard matches the approved responsive visual baseline", async ({ page }) => {
  test.skip(process.platform !== "darwin", "Levelogのvisual baselineは配布対象のmacOSで検証します");
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "最初の活動を記録しましょう" })).toBeVisible();
  await expect(page).toHaveScreenshot("dashboard-empty.png", {
    fullPage: true,
    animations: "disabled",
    maxDiffPixels: 5_000,
  });
});
