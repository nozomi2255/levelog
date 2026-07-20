import { useQuery, useQueryClient } from "@tanstack/react-query";
import { BrowserRouter, Navigate, Route, Routes, useLocation, useNavigate } from "react-router-dom";
import { AppShell } from "../components/app-shell";
import { HudBadge, HudPanel } from "../components/hud";
import { ActivitiesPage, ActivityInboxPage, AnalysisPage, NewActivityPage } from "../features/activities";
import { DashboardPage } from "../features/dashboard/DashboardPage";
import { OnboardingPage } from "../features/onboarding";
import { QuestsPage } from "../features/quests/QuestsPage";
import { SettingsPage } from "../features/settings";
import { SkillsPage } from "../features/skills/SkillsPage";
import { api } from "../lib/api";

function NextPhasePage({ title, message }: { title: string; message: string }) {
  return <HudPanel title={title} className="page-placeholder"><div className="empty-state"><HudBadge tone="purple">NEXT PHASE</HudBadge><h1>{title}</h1><p>{message}</p></div></HudPanel>;
}

function OnboardingRoute() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const onSaved = async () => {
    await queryClient.invalidateQueries({ queryKey: ["boot-state"] });
    navigate("/", { replace: true });
  };
  return <main className="onboarding-layout"><OnboardingPage onSaved={() => void onSaved()} /></main>;
}

function AppRoutes() {
  const location = useLocation();
  const boot = useQuery({ queryKey: ["boot-state"], queryFn: api.getBootState, retry: 0 });

  if (boot.isPending) return <main className="boot-state" role="status">Levelogを起動しています…</main>;
  if (boot.isError) return <main className="boot-state" role="alert"><h1>Levelogを起動できませんでした</h1><p>ローカルデータベースを開けません。アプリを再起動してください。</p></main>;

  if (!boot.data.onboardingComplete && location.pathname !== "/onboarding") {
    return <Navigate to="/onboarding" replace />;
  }
  if (boot.data.onboardingComplete && location.pathname === "/onboarding") {
    return <Navigate to="/" replace />;
  }

  return <Routes>
    <Route path="/onboarding" element={<OnboardingRoute />} />
    <Route element={<AppShell />}>
      <Route index element={<DashboardPage />} />
      <Route path="quests" element={<QuestsPage />} />
      <Route path="activities" element={<ActivitiesPage />} />
      <Route path="activities/inbox" element={<ActivityInboxPage />} />
      <Route path="activities/new" element={<NewActivityPage />} />
      <Route path="activities/:activityId/analysis" element={<AnalysisPage />} />
      <Route path="skills" element={<SkillsPage />} />
      <Route path="reports" element={<NextPhasePage title="レポート" message="週次レビューは次フェーズで提供します。活動・証拠・振り返りのデータは、それまでローカルに蓄積されます。" />} />
      <Route path="memory" element={<NextPhasePage title="メモリ" message="検索できる実績メモリは次フェーズで提供します。現在の全記録は、設定からJSONとして取り出せます。" />} />
      <Route path="settings" element={<SettingsPage />} />
    </Route>
    <Route path="*" element={<Navigate to="/" replace />} />
  </Routes>;
}

export function App() {
  return <BrowserRouter><AppRoutes /></BrowserRouter>;
}
