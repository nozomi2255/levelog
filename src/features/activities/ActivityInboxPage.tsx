import { useQuery } from "@tanstack/react-query";
import { Link, useLocation } from "react-router-dom";
import { HudBadge, HudPanel } from "../../components/hud";
import { api } from "../../lib/api";

const labels: Record<string, { label: string; tone: "cyan" | "gold" | "green" | "purple"; detail: string }> = {
  captured: { label: "未整理", tone: "cyan", detail: "余裕があるときに分析を始められます" },
  analysis_running: { label: "分析中", tone: "purple", detail: "結果を待っています" },
  needs_input: { label: "質問待ち", tone: "gold", detail: "答えられる範囲で一問だけ回答できます" },
  assessable: { label: "確認待ち", tone: "gold", detail: "AIの整理案を確認してください" },
  review_pending: { label: "確認待ち", tone: "gold", detail: "AIの整理案を確認してください" },
  confirmed: { label: "確定済み", tone: "green", detail: "証拠として保存されています" },
  excluded: { label: "評価対象外", tone: "purple", detail: "原文は保持されています" },
};

export function ActivityInboxPage() {
  const location = useLocation();
  const inbox = useQuery({ queryKey: ["activity-inbox"], queryFn: api.listActivityInbox });
  if (inbox.isPending) return <HudPanel title="整理インボックス"><p role="status">経験を読み込んでいます…</p></HudPanel>;
  if (inbox.isError) return <HudPanel title="整理インボックス"><p role="alert">{String(inbox.error)}</p></HudPanel>;
  return <section className="feature-page feature-page--narrow" aria-labelledby="inbox-title"><header className="page-heading"><div><p className="hud-kicker">REFLECTION INBOX</p><h1 id="inbox-title">整理インボックス</h1><p>急いでいるときに残した経験を、今の余裕に合わせて一件ずつ整えます。</p></div><Link to="/activities/new" className="hud-button hud-button--gold">経験を記録</Link></header>
    {location.state && <p className="result-message" role="status">原文をローカルに保存しました。AIを使わなくても、ここからいつでも整理できます。</p>}
    <HudPanel title={`${inbox.data.length}件の経験`}>{inbox.data.length === 0 ? <div className="empty-state"><HudBadge tone="green">CLEAR</HudBadge><h2>整理待ちの経験はありません</h2><p>新しい経験を一言で残しておくと、あとでここに届きます。</p><Link className="hud-button hud-button--gold" to="/activities/new">経験を記録する</Link></div> : <ul className="inbox-list">{inbox.data.map((item) => { const state = labels[item.workflow.state] ?? labels.captured!; return <li key={item.id}><Link to={`/activities/${item.id}/analysis`}><div><HudBadge tone={state.tone}>{state.label}</HudBadge><strong>{item.actionText || item.challengeText || item.outcomeText || "活動の記録"}</strong><small>{item.occurredOn} · {state.detail}</small></div><span aria-hidden="true">→</span></Link></li>; })}</ul>}</HudPanel>
  </section>;
}
