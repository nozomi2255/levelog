import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type Dispatch, type SetStateAction } from "react";
import { Link, useParams } from "react-router-dom";
import { HudBadge, HudButton, HudPanel } from "../../components/hud";
import { api } from "../../lib/api";
import type { CandidateDecisionInput, SkillCandidateDto } from "../../lib/types";

type Draft = { decision: CandidateDecisionInput["decision"]; reason: string; evidence: string };
const toDraft = (candidate: SkillCandidateDto): Draft => ({ decision: candidate.decision === "pending" ? "rejected" : candidate.decision, reason: candidate.reason, evidence: candidate.evidence });

export function AnalysisPage() {
  const { activityId = "" } = useParams();
  const client = useQueryClient();
  const [payload, setPayload] = useState("");
  const [startedAnalysisId, setStartedAnalysisId] = useState<string | null>(null);
  const [ignorePrevious, setIgnorePrevious] = useState(false);
  const [drafts, setDrafts] = useState<Record<string, Draft>>({});
  const activity = useQuery({ queryKey: ["activity", activityId], queryFn: () => api.getActivity(activityId), enabled: Boolean(activityId) });
  const preview = useQuery({ queryKey: ["analysis-preview", activityId], queryFn: () => api.getAnalysisPreview(activityId), enabled: Boolean(activityId) });
  const existingAnalysisId = ignorePrevious ? null : activity.data?.analyses[0]?.id ?? null;
  const analysisId = startedAnalysisId ?? existingAnalysisId;
  const analysis = useQuery({ queryKey: ["analysis", analysisId], queryFn: () => api.getActivityAnalysis(analysisId!), enabled: Boolean(analysisId), refetchInterval: (query) => ["pending", "running"].includes(query.state.data?.status ?? "") ? 1500 : false });
  const submittedPayload = payload || preview.data?.submittedPayload || "";
  const start = useMutation({ mutationFn: () => api.startActivityAnalysis({ activityId, submittedPayload }), onSuccess: (job) => { setStartedAnalysisId(job.id); setIgnorePrevious(false); } });
  const cancel = useMutation({ mutationFn: () => api.cancelActivityAnalysis(analysisId!), onSuccess: () => void client.invalidateQueries({ queryKey: ["analysis", analysisId] }) });
  const confirm = useMutation({ mutationFn: () => api.confirmActivityAnalysis({ analysisId: analysisId!, candidateDecisions: (analysis.data?.skillCandidates ?? []).map((candidate) => { const draft = drafts[candidate.id] ?? toDraft(candidate); return { candidateId: candidate.id, decision: draft.decision, editedReason: draft.decision === "edited" ? draft.reason : null, editedEvidence: draft.decision === "edited" ? draft.evidence : null }; }) }), onSuccess: () => { void client.invalidateQueries({ queryKey: ["analysis", analysisId] }); void client.invalidateQueries({ queryKey: ["dashboard"] }); void client.invalidateQueries({ queryKey: ["skills"] }); } });
  const quest = useMutation({ mutationFn: () => api.generateQuest({ activityId, analysisId: analysisId! }), onSuccess: () => { void client.invalidateQueries({ queryKey: ["quests"] }); void client.invalidateQueries({ queryKey: ["dashboard"] }); } });
  const retry = () => { setStartedAnalysisId(null); setIgnorePrevious(true); setDrafts({}); };

  if (activity.isPending || preview.isPending) return <HudPanel title="AI分析"><p className="empty-copy" role="status">送信内容を準備しています…</p></HudPanel>;
  if (activity.isError || preview.isError) return <HudPanel title="AI分析"><p role="alert">{String(activity.error ?? preview.error)}</p></HudPanel>;
  const result = analysis.data;
  return <section className="feature-page feature-page--narrow" aria-labelledby="analysis-title"><header className="page-heading"><div><p className="hud-kicker">HUMAN CONFIRMATION REQUIRED</p><h1 id="analysis-title">AI分析の確認</h1><p>AIの提案は候補です。採用・編集・却下を選ぶまで証拠や分析XPには反映されません。</p></div><Link to="/activities">一覧へ戻る</Link></header>
    {!analysisId && <HudPanel title="Codexへ送信する内容"><p className="cloud-notice"><HudBadge tone="gold">CLOUD</HudBadge> {preview.data.cloudInferenceNotice}</p><label className="field">送信するJSON<textarea className="json-editor" aria-label="送信するJSON" value={submittedPayload} onChange={(event) => setPayload(event.target.value)} rows={14} spellCheck={false} /></label><div className="form-actions"><HudButton tone="gold" onClick={() => start.mutate()} disabled={start.isPending}>{start.isPending ? "送信中…" : "内容を確認して送信"}</HudButton></div>{start.error && <p role="alert">{String(start.error)}</p>}</HudPanel>}
    {analysisId && analysis.isPending && <HudPanel title="AI分析"><p role="status">分析状態を読み込んでいます…</p></HudPanel>}
    {analysis.isError && <HudPanel title="AI分析"><p role="alert">{String(analysis.error)}</p><HudButton onClick={() => void analysis.refetch()}>再読込</HudButton></HudPanel>}
    {result && <AnalysisResult result={result} drafts={drafts} setDrafts={setDrafts} onConfirm={() => confirm.mutate()} confirming={confirm.isPending} onCancel={() => cancel.mutate()} cancelling={cancel.isPending} onRetry={retry} onGenerate={() => quest.mutate()} generating={quest.isPending} />}
    {confirm.data && <p className="result-message" role="status">{confirm.data.confirmedObservationCount}件の証拠を確定し、{confirm.data.xpAwarded} XPを追加しました。</p>}
    {quest.data && <p className="result-message" role="status">クエスト「{quest.data.title}」を作成しました。</p>}
    {confirm.error && <p role="alert">{String(confirm.error)}</p>}{quest.error && <p role="alert">{String(quest.error)}</p>}
  </section>;
}

function AnalysisResult({ result, drafts, setDrafts, onConfirm, confirming, onCancel, cancelling, onRetry, onGenerate, generating }: { result: Awaited<ReturnType<typeof api.getActivityAnalysis>>; drafts: Record<string, Draft>; setDrafts: Dispatch<SetStateAction<Record<string, Draft>>>; onConfirm: () => void; confirming: boolean; onCancel: () => void; cancelling: boolean; onRetry: () => void; onGenerate: () => void; generating: boolean }) {
  if (["pending", "running"].includes(result.status)) return <HudPanel title="Codexで分析中"><div className="analysis-running"><span className="scan-indicator" aria-hidden="true" /><p role="status">分析中です。画面を閉じても活動の原文は保存されています。</p><HudButton tone="purple" onClick={onCancel} disabled={cancelling}>{cancelling ? "取消中…" : "分析をキャンセル"}</HudButton></div></HudPanel>;
  if (result.status === "failed" || result.status === "cancelled") return <HudPanel title="分析を完了できませんでした"><p role="alert">{result.errorMessage ?? "分析を完了できませんでした。"}</p><p className="empty-copy">活動は保持されています。送信内容を確認して、新しい分析として再試行できます。</p><HudButton tone="gold" onClick={onRetry}>送信内容に戻って再試行</HudButton></HudPanel>;
  if (result.status === "confirmed") return <HudPanel title="分析を確定しました"><HudBadge tone="green">CONFIRMED</HudBadge><p>確認済みの証拠がスキル観測へ反映されています。</p><HudButton tone="purple" onClick={onGenerate} disabled={generating}>{generating ? "生成中…" : "次のクエストを生成"}</HudButton></HudPanel>;
  return <HudPanel title="分析候補"><div className="analysis-summary"><HudBadge tone="purple">AI SUGGESTION</HudBadge>{result.summary && <p>{result.summary}</p>}{result.outcomes.length > 0 && <><h3>観測された成果</h3><ul>{result.outcomes.map((outcome) => <li key={outcome}>{outcome}</li>)}</ul></>}{result.missingInformationQuestion && <p className="missing-question"><strong>不足情報：</strong>{result.missingInformationQuestion}</p>}</div><div className="candidate-list">{result.skillCandidates.map((candidate) => { const draft = drafts[candidate.id] ?? toDraft(candidate); const update = (patch: Partial<Draft>) => setDrafts((current) => ({ ...current, [candidate.id]: { ...draft, ...patch } })); return <fieldset className="candidate-card" key={candidate.id}><legend>{candidate.skillId} <span>確度 {Math.round(candidate.confidence * 100)}%</span></legend><div className="candidate-decisions"><label><input name={`decision-${candidate.id}`} type="radio" checked={draft.decision === "accepted"} onChange={() => update({ decision: "accepted" })} />採用</label><label><input name={`decision-${candidate.id}`} type="radio" checked={draft.decision === "edited"} onChange={() => update({ decision: "edited" })} />編集して採用</label><label><input name={`decision-${candidate.id}`} type="radio" checked={draft.decision === "rejected"} onChange={() => update({ decision: "rejected" })} />却下</label></div><label className="field">候補の理由<textarea disabled={draft.decision !== "edited"} value={draft.reason} onChange={(event) => update({ reason: event.target.value })} /></label><label className="field">証拠<textarea disabled={draft.decision !== "edited"} value={draft.evidence} onChange={(event) => update({ evidence: event.target.value })} /></label></fieldset>; })}</div><div className="form-actions"><HudButton tone="green" onClick={onConfirm} disabled={confirming}>{confirming ? "確定中…" : "判断を確定する（+20 XP）"}</HudButton></div></HudPanel>;
}
