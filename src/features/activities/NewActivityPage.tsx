import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { Link, useNavigate } from "react-router-dom";
import { HudButton, HudPanel } from "../../components/hud";
import { api } from "../../lib/api";

export function NewActivityPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [occurredOn, setOccurredOn] = useState(() => new Date().toISOString().slice(0, 10));
  const [actionText, setActionText] = useState("");
  const [challengeText, setChallengeText] = useState("");
  const [outcomeText, setOutcomeText] = useState("");
  const hasContent = Boolean(actionText.trim() || challengeText.trim() || outcomeText.trim());
  const create = useMutation({ mutationFn: () => api.createActivity({ occurredOn, actionText, challengeText, outcomeText }), onSuccess: (activity) => { void queryClient.invalidateQueries({ queryKey: ["activities"] }); void queryClient.invalidateQueries({ queryKey: ["dashboard"] }); navigate(`/activities/${activity.id}/analysis`); } });
  const submit = (event: FormEvent) => { event.preventDefault(); create.mutate(); };
  return <section className="feature-page feature-page--narrow" aria-labelledby="new-activity-title"><header className="page-heading"><div><p className="hud-kicker">QUICK CAPTURE · +10 XP</p><h1 id="new-activity-title">活動を記録</h1><p>空欄があっても大丈夫です。事実として残せる項目を一つ入力してください。</p></div><Link to="/activities">一覧へ戻る</Link></header><HudPanel title="今日の成長の証拠"><form className="feature-form" onSubmit={submit}><label className="field">日付<input required type="date" value={occurredOn} onChange={(event) => setOccurredOn(event.target.value)} /></label><label className="field">何をした？<textarea value={actionText} onChange={(event) => setActionText(event.target.value)} placeholder="例：曖昧な依頼を三つの確認事項に分けた" /></label><label className="field">何が難しかった？<textarea value={challengeText} onChange={(event) => setChallengeText(event.target.value)} placeholder="例：関係者ごとに前提が違っていた" /></label><label className="field">何が変わった？<textarea value={outcomeText} onChange={(event) => setOutcomeText(event.target.value)} placeholder="例：実装前に認識のずれを解消できた" /></label>{create.error && <p role="alert">{String(create.error)}</p>}<div className="form-actions"><HudButton tone="gold" disabled={!hasContent || create.isPending} type="submit">{create.isPending ? "保存中…" : "保存して送信内容を確認"}</HudButton></div></form></HudPanel></section>;
}
