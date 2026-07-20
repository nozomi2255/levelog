import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { HudBadge, HudPanel } from "../../components/hud";
import { api } from "../../lib/api";

const statusLabel: Record<string, string> = { pending: "待機中", running: "分析中", succeeded: "確認待ち", failed: "失敗", cancelled: "取消", confirmed: "確定済み" };

export function ActivitiesPage() {
  const activities = useQuery({ queryKey: ["activities"], queryFn: api.listActivities });
  if (activities.isPending) return <HudPanel title="アクティビティ"><p className="empty-copy" role="status">活動を読み込んでいます…</p></HudPanel>;
  if (activities.isError) return <HudPanel title="アクティビティ"><p role="alert">{String(activities.error)}</p></HudPanel>;
  return <section className="feature-page" aria-labelledby="activities-title">
    <header className="page-heading"><div><p className="hud-kicker">EVIDENCE LOG</p><h1 id="activities-title">アクティビティ</h1><p>経験を一言で残し、余裕があるときにAIと短く整理します。</p></div><div className="page-heading__actions"><Link className="hud-button hud-button--cyan" to="/activities/inbox">整理インボックス</Link><Link className="hud-button hud-button--gold" to="/activities/new">経験を記録する</Link></div></header>
    <HudPanel title={`記録 ${activities.data.length}件`}>
      {activities.data.length === 0 ? <div className="empty-state"><HudBadge>EMPTY</HudBadge><h2>まだ活動はありません</h2><p>一文だけでも、あとから成長の証拠として整理できます。</p><Link className="hud-button hud-button--gold" to="/activities/new">最初の経験を記録</Link></div> : <ul className="activity-list">{activities.data.map((activity) => <li key={activity.id}><Link to={`/activities/${activity.id}/analysis`}><span><strong>{activity.actionText || activity.challengeText || activity.outcomeText || "活動の記録"}</strong><small>{activity.occurredOn}</small></span><HudBadge tone={activity.analysisStatus === "confirmed" ? "green" : activity.analysisStatus === "failed" ? "gold" : "cyan"}>{activity.analysisStatus ? statusLabel[activity.analysisStatus] ?? activity.analysisStatus : "未整理"}</HudBadge></Link></li>)}</ul>}
    </HudPanel>
  </section>;
}
