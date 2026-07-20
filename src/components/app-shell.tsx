import { Bell, BookOpen, BrainCircuit, ClipboardList, House, ScrollText, Settings, Swords } from "lucide-react";
import { NavLink, Outlet } from "react-router-dom";

const navigation = [
  ["/", "ホーム", "HOME", House], ["/quests", "クエスト", "QUESTS", Swords], ["/activities", "アクティビティ", "ACTIVITIES", ClipboardList], ["/skills", "スキル", "SKILLS", BrainCircuit], ["/reports", "レポート", "REPORTS", ScrollText], ["/memory", "メモリ", "MEMORY", BookOpen], ["/settings", "設定", "SETTINGS", Settings],
] as const;

export function AppShell() {
  return <div className="app-shell">
    <aside className="sidebar"><NavLink className="brand" to="/" aria-label="Levelog ホーム">LEVEL·LOG</NavLink><nav aria-label="メインナビゲーション">{navigation.map(([to, label, sublabel, Icon]) => <NavLink key={to} to={to} end={to === "/"} aria-label={label} className={({ isActive }) => `nav-item ${isActive ? "nav-item--active" : ""}`}><Icon aria-hidden="true" /><span>{label}<small>{sublabel}</small></span></NavLink>)}</nav><p className="sidebar__footer">LOCAL GROWTH SYSTEM</p></aside>
    <div className="app-shell__content"><header className="topbar"><div><p className="hud-kicker">TODAY · LOCAL MODE</p><p className="topbar__message">今日も、成長の証拠を見つけよう。</p></div><div className="topbar__actions"><button className="icon-button" aria-label="通知（次フェーズ）" title="通知は次フェーズで提供します" disabled><Bell aria-hidden="true" /></button></div></header><main className="app-main"><Outlet /></main></div>
  </div>;
}
