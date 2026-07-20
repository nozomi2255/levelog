import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { RadarChart, WeeklyLineChart } from "../../components/charts";
import { GrowthCore } from "../../components/growth-core";
import { HudBadge, HudButton, HudPanel, ProgressBar } from "../../components/hud";
import { api } from "../../lib/api";

const categoryLabels: Record<string, string> = {
  thinking: "思考力", technical: "技術力", communication: "伝える力", execution: "実行力", interpersonal: "対人力",
};

export function DashboardPage() {
  const dashboard = useQuery({ queryKey: ["dashboard"], queryFn: api.getDashboard });
  if (dashboard.isLoading) return <HudPanel title="成長管制画面"><p className="empty-copy" role="status">成長データを読み込んでいます…</p></HudPanel>;
  if (dashboard.isError) return <HudPanel title="成長管制画面"><div className="empty-state"><h1>成長データを読み込めませんでした</h1><p>ローカルデータベースへの接続を確認して、もう一度試してください。</p><HudButton onClick={() => void dashboard.refetch()}>再試行する</HudButton></div></HudPanel>;
  const data = dashboard.data;
  if (!data) return null;
  const currentLevelFloor = 50 * (data.level - 1) * data.level;
  const levelProgress = Math.max(0, data.totalXp - currentLevelFloor);
  const radar = data.categoryObservations.map(({ category, count }) => ({ label: categoryLabels[category] ?? category, value: count, max: Math.max(3, ...data.categoryObservations.map((item) => item.count)) }));
  return <div className="dashboard">
    <HudPanel title="現在のステータス" className="status-panel"><div className="status-grid"><div><p className="hud-kicker">ACCOUNT LEVEL</p><p className="level-value">{data.level}</p></div><div><p className="hud-kicker">総獲得XP {data.totalXp.toLocaleString()}</p><ProgressBar label="次のレベルまでの経験値" value={levelProgress} max={Math.max(levelProgress + data.xpToNextLevel, 1)} /></div><RadarChart data={radar} /></div></HudPanel>
    <HudPanel title="今日のクエスト" className="quests-panel">{data.activeQuest ? <QuestSummary title={data.activeQuest.title} status={data.activeQuest.status} description={data.activeQuest.description} /> : <div className="empty-state"><HudBadge tone="purple">NO ACTIVE QUEST</HudBadge><h1>最初の活動を記録しましょう</h1><p>活動分析を承認すると、安全なクエスト候補を受け取れます。</p><Link className="hud-button hud-button--gold" to="/activities/new">活動を記録する</Link></div>}</HudPanel>
    <GrowthCore value={data.todayXp} observations={data.todayObservations} />
    <HudPanel title="最近のアクティビティ" className="activities-panel">{data.recentActivities.length ? <ul aria-label="最近のアクティビティ">{data.recentActivities.map((activity) => <li key={activity.id}><strong>{activity.actionText || "活動を記録"}</strong><br /><small>{activity.occurredOn} · {activity.analysisStatus ?? "未分析"}</small></li>)}</ul> : <p className="empty-copy">まだ承認済みのアクティビティはありません。</p>}</HudPanel>
    <HudPanel title="今日の観測" className="skills-panel"><p className="level-value">{data.todayObservations}</p><p className="empty-copy">活動 {data.todayActivities} 件から、ユーザーが承認した成長の証拠です。</p><Link to="/skills">スキルの観測を見る</Link></HudPanel>
    <HudPanel title="AIからの推薦クエスト" className="recommendation-panel">{data.activeQuest ? <QuestSummary title={data.activeQuest.title} status={data.activeQuest.status} description={`${data.activeQuest.targetSkillId} · ${data.activeQuest.estimatedMinutes}分`} /> : <p className="empty-copy">分析結果を承認すると、ここに安全な次の一歩が表示されます。</p>}</HudPanel>
    <HudPanel title="成長ログ（直近7日）" className="log-panel"><WeeklyLineChart points={data.weeklyXp} /></HudPanel>
  </div>;
}

function QuestSummary({ title, status, description }: { title: string; status: string; description: string }) { return <div><HudBadge tone={status === "completed" ? "green" : "gold"}>{status}</HudBadge><h3>{title}</h3><p className="empty-copy">{description}</p><Link to="/quests">クエストを確認する</Link></div>; }
