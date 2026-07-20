import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { HudBadge, HudButton, HudPanel } from "../../components/hud";
import { api } from "../../lib/api";
import type { QuestDto, QuestReflectionResult, QuestStatus, QuestTransitionAction } from "../../lib/types";

const statusLabel: Record<QuestStatus, string> = { proposed: "提案", accepted: "受注済み", in_progress: "進行中", completed: "完了", rescheduled: "延期", adjusted: "縮小・調整", cancelled: "破棄" };
const actionLabel: Record<QuestTransitionAction, string> = { accept: "受注する", start: "開始する", complete: "完了にする", reschedule: "延期する", adjust: "縮小する", cancel: "破棄する" };
const actionsFor: Record<QuestStatus, QuestTransitionAction[]> = { proposed: ["accept", "reschedule", "adjust", "cancel"], accepted: ["start", "reschedule", "adjust", "cancel"], in_progress: ["complete", "reschedule", "adjust", "cancel"], completed: [], rescheduled: ["accept", "start", "cancel"], adjusted: ["accept", "start", "cancel"], cancelled: [] };

export function QuestsPage() {
  const client = useQueryClient();
  const [reflectionFor, setReflectionFor] = useState<string | null>(null);
  const quests = useQuery({ queryKey: ["quests"], queryFn: api.listQuests });
  const invalidate = async () => { await client.invalidateQueries({ queryKey: ["quests"] }); await client.invalidateQueries({ queryKey: ["dashboard"] }); };
  const transition = useMutation({ mutationFn: api.transitionQuest, onSuccess: invalidate });
  const reflect = useMutation({ mutationFn: api.saveQuestReflection, onSuccess: async () => { setReflectionFor(null); await invalidate(); } });
  if (quests.isLoading) return <HudPanel title="クエスト"><p className="empty-copy" role="status">クエストを読み込んでいます…</p></HudPanel>;
  if (quests.isError) return <HudPanel title="クエスト"><div className="empty-state"><h1>クエストを読み込めませんでした</h1><HudButton onClick={() => void quests.refetch()}>再試行する</HudButton></div></HudPanel>;
  const questList = quests.data;
  if (!questList) return null;

  return <section className="feature-page" aria-labelledby="quests-title"><header className="page-heading"><div><p className="hud-kicker">SAFE NEXT CHALLENGE</p><h1 id="quests-title">クエスト</h1><p>今の状況より少しだけ難しい挑戦です。延期・縮小・破棄を失敗として扱いません。</p></div><Link className="hud-button hud-button--gold" to="/activities">活動から提案を作る</Link></header>
    {transition.isError && <p role="alert">状態を変更できませんでした: {String(transition.error)}</p>}{reflect.isError && <p role="alert">振り返りを保存できませんでした: {String(reflect.error)}</p>}{reflect.data && <p className="result-message" role="status">振り返りを保存し、{reflect.data.xpAwarded} XPを追加しました。</p>}
    <div className="quest-grid">{questList.length ? questList.map((quest) => <QuestCard key={quest.id} quest={quest} transition={transition.mutate} pending={transition.isPending} reflectionFor={reflectionFor} setReflectionFor={setReflectionFor} reflect={reflect.mutate} reflecting={reflect.isPending} />) : <HudPanel title="クエスト"><div className="empty-state"><HudBadge tone="purple">NO QUEST</HudBadge><h2>まだクエストはありません</h2><p>活動のAI分析を確認・確定すると、その文脈から安全な次の一歩を提案できます。</p><Link className="hud-button hud-button--gold" to="/activities/new">活動を記録する</Link></div></HudPanel>}</div>
  </section>;
}

function QuestCard({ quest, transition, pending, reflectionFor, setReflectionFor, reflect, reflecting }: { quest: QuestDto; transition: (input: { questId: string; action: QuestTransitionAction; scheduledOn: string | null; estimatedMinutes: number | null }) => void; pending: boolean; reflectionFor: string | null; setReflectionFor: (id: string | null) => void; reflect: (input: { questId: string; result: QuestReflectionResult; learned: string; difficultyActual: number | null; nextAction: string }) => void; reflecting: boolean }) {
  const [scheduledOn, setScheduledOn] = useState(quest.scheduledOn ?? "");
  const [minutes, setMinutes] = useState(String(quest.estimatedMinutes));
  const [result, setResult] = useState<QuestReflectionResult>("completed");
  const [learned, setLearned] = useState("");
  const [difficultyActual, setDifficultyActual] = useState(String(quest.difficulty));
  const [nextAction, setNextAction] = useState("");
  const [confirmingCancel, setConfirmingCancel] = useState(false);
  const apply = (action: QuestTransitionAction) => transition({ questId: quest.id, action, scheduledOn: action === "reschedule" ? (scheduledOn || null) : null, estimatedMinutes: action === "adjust" ? Number(minutes) : null });
  const tone = quest.status === "completed" ? "green" : quest.status === "proposed" ? "purple" : "gold";

  return <HudPanel title={quest.title} className="quest-card"><HudBadge tone={tone}>{statusLabel[quest.status]}</HudBadge><p>{quest.description}</p><p className="empty-copy">対象: {quest.targetSkillId} · 予定: {quest.estimatedMinutes}分 · 難易度: {quest.difficulty}/5</p><h3>達成条件</h3><ul>{quest.successCriteria.map((criterion) => <li key={criterion}>{criterion}</li>)}</ul><p className="empty-copy">記録する証拠: {quest.evidencePrompt}</p>
    {actionsFor[quest.status].includes("reschedule") && <label className="field">延期日<input name={`scheduledOn-${quest.id}`} autoComplete="off" type="date" value={scheduledOn} onChange={(event) => setScheduledOn(event.target.value)} /></label>}{actionsFor[quest.status].includes("adjust") && <label className="field">縮小後の予定時間（5〜30分）<input name={`estimatedMinutes-${quest.id}`} autoComplete="off" type="number" min="5" max="30" value={minutes} onChange={(event) => setMinutes(event.target.value)} /></label>}
    <div className="quest-actions">{actionsFor[quest.status].map((action) => <HudButton key={action} type="button" disabled={pending || (action === "reschedule" && !scheduledOn)} tone={action === "complete" ? "green" : action === "accept" || action === "start" ? "gold" : "cyan"} className={action === "cancel" ? "button-danger" : ""} onClick={() => action === "cancel" ? setConfirmingCancel(true) : apply(action)}>{actionLabel[action]}</HudButton>)}</div>
    {confirmingCancel && <div className="destructive-confirmation" role="alert"><p>このクエストは履歴に残りますが、破棄後は再開できません。</p><div className="form-actions"><HudButton className="button-danger" type="button" onClick={() => { setConfirmingCancel(false); apply("cancel"); }} disabled={pending}>本当に破棄する</HudButton><HudButton type="button" onClick={() => setConfirmingCancel(false)} disabled={pending}>戻る</HudButton></div></div>}
    {quest.status === "completed" && <HudButton type="button" tone="green" onClick={() => setReflectionFor(reflectionFor === quest.id ? null : quest.id)}>振り返りを記録する</HudButton>}
    {reflectionFor === quest.id && <form className="feature-form" onSubmit={(event) => { event.preventDefault(); reflect({ questId: quest.id, result, learned, difficultyActual: Number(difficultyActual), nextAction }); }}><label className="field">結果<select name={`result-${quest.id}`} autoComplete="off" value={result} onChange={(event) => setResult(event.target.value as QuestReflectionResult)}><option value="completed">完了</option><option value="partially_completed">一部完了</option><option value="not_completed">未完了</option><option value="rested">休息した</option></select></label><label className="field">実際の難易度（1〜5）<input name={`difficulty-${quest.id}`} autoComplete="off" type="number" min="1" max="5" value={difficultyActual} onChange={(event) => setDifficultyActual(event.target.value)} /></label><label className="field">学び<textarea name={`learned-${quest.id}`} autoComplete="off" value={learned} onChange={(event) => setLearned(event.target.value)} /></label><label className="field">次の一歩<textarea name={`nextAction-${quest.id}`} autoComplete="off" value={nextAction} onChange={(event) => setNextAction(event.target.value)} /></label><HudButton tone="green" disabled={reflecting}>振り返りを保存する（+40 XP）</HudButton></form>}
  </HudPanel>;
}
