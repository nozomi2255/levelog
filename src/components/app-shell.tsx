import { useQuery } from "@tanstack/react-query";
import { Bell, BrainCircuit, ClipboardList, Database, House, Plus, ScrollText, Settings, Swords } from "lucide-react";
import { NavLink, Outlet } from "react-router-dom";
import { api } from "../lib/api";

const navigation = [
  ["/", "ホーム", "HOME", House], ["/quests", "クエスト", "QUESTS", Swords], ["/activities", "アクティビティ", "ACTIVITIES", ClipboardList], ["/skills", "スキル", "SKILLS", BrainCircuit], ["/reports", "レポート", "REPORTS", ScrollText], ["/memory", "エビデンス", "EVIDENCE", Database], ["/settings", "設定", "SETTINGS", Settings],
] as const;

export function AppShell() {
  const profile = useQuery({ queryKey: ["user-profile"], queryFn: api.getUserProfile, retry: false });
  return <div className="app-shell">
    <a className="skip-link" href="#main-content">本文へスキップ</a>
    <aside className="sidebar"><NavLink className="brand" to="/" aria-label="Levelog ホーム">LEVEL·LOG</NavLink><NavLink className="sidebar-capture" to="/activities/new"><Plus aria-hidden="true" />経験を記録</NavLink><nav aria-label="メインナビゲーション">{navigation.map(([to, label, sublabel, Icon]) => <NavLink key={to} to={to} end={to === "/"} aria-label={label} className={({ isActive }) => `nav-item ${isActive ? "nav-item--active" : ""}`}><Icon aria-hidden="true" /><span>{label}<small>{sublabel}</small></span></NavLink>)}</nav>{profile.data && <div className="sidebar-profile"><p className="hud-kicker">PROFILE</p><strong>{profile.data.role}</strong>{profile.data.growthGoal && <small>{profile.data.growthGoal}</small>}</div>}<p className="sidebar__footer">LOCAL GROWTH SYSTEM</p></aside>
    <div className="app-shell__content"><header className="topbar"><div><p className="hud-kicker">TODAY · LOCAL MODE</p><p className="topbar__message">今日も、成長の証拠を見つけよう。</p></div><div className="topbar__actions"><NavLink className="topbar-capture" to="/activities/new"><Plus aria-hidden="true" />経験を記録</NavLink><button className="icon-button" aria-label="通知（次フェーズ）" title="通知は次フェーズで提供します" disabled><Bell aria-hidden="true" /></button></div></header><main id="main-content" className="app-main" tabIndex={-1}><Outlet /></main></div>
  </div>;
}
