import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { Link, useNavigate } from "react-router-dom";
import { HudBadge, HudButton, HudPanel } from "../../components/hud";
import { api } from "../../lib/api";
import type { CaptureMode } from "../../lib/types";

const modes: Array<{ value: CaptureMode; title: string; detail: string }> = [
  { value: "quick", title: "10秒で記録", detail: "今は一言だけ残す" },
  { value: "guided", title: "1分で整理", detail: "あとでAIの短い質問に答える" },
  { value: "deep", title: "深掘りする", detail: "保存後、分析の確認へ進む" },
];

const todayInLocalTime = () => {
  const now = new Date();
  return new Date(now.getTime() - now.getTimezoneOffset() * 60_000).toISOString().slice(0, 10);
};

export function NewActivityPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [occurredOn, setOccurredOn] = useState(todayInLocalTime);
  const [rawText, setRawText] = useState("");
  const [captureMode, setCaptureMode] = useState<CaptureMode>("quick");
  const capture = useMutation({
    mutationFn: () => api.quickCaptureActivity({ occurredOn, rawText, captureMode }),
    onSuccess: (activity) => {
      void queryClient.invalidateQueries({ queryKey: ["activities"] });
      void queryClient.invalidateQueries({ queryKey: ["activity-inbox"] });
      void queryClient.invalidateQueries({ queryKey: ["dashboard"] });
      navigate(captureMode === "deep" ? `/activities/${activity.id}/analysis` : "/activities/inbox", { state: { captured: true } });
    },
  });
  const submit = (event: FormEvent) => { event.preventDefault(); capture.mutate(); };

  return <section className="feature-page feature-page--narrow" aria-labelledby="new-activity-title">
    <header className="page-heading"><div><p className="hud-kicker">EXPERIENCE CAPTURE · +10 XP</p><h1 id="new-activity-title">今日、何がありましたか？</h1><p>整った文章は不要です。できたこと、困ったこと、判断したことを一言のまま残せます。</p></div><Link to="/activities">一覧へ戻る</Link></header>
    <HudPanel title="経験を預ける">
      <form className="feature-form" onSubmit={submit}>
        <label className="field">いつの経験？<input name="occurredOn" autoComplete="off" required type="date" value={occurredOn} onChange={(event) => setOccurredOn(event.target.value)} /></label>
        <label className="field">起きたこと
          <textarea name="rawText" autoComplete="off" autoFocus maxLength={20_000} value={rawText} onChange={(event) => setRawText(event.target.value)} placeholder="例：API遅かったからSQL直した。レビューで設計を見直した…" aria-describedby="capture-help" />
        </label>
        <p id="capture-help" className="form-help">原文はこのまま不変で保存されます。AIを使わない場合も、あとで整理できます。</p>
        <fieldset className="capture-mode"><legend>今使える時間</legend>{modes.map((mode) => <label key={mode.value} className="capture-mode__option"><input type="radio" name="capture-mode" value={mode.value} checked={captureMode === mode.value} onChange={() => setCaptureMode(mode.value)} /><span><strong>{mode.title}</strong><small>{mode.detail}</small></span></label>)}</fieldset>
        <div className="capture-assurance"><HudBadge tone="green">SAVED LOCALLY</HudBadge><span>保存時に +10 XP。AI分析の成否に関わらず、経験は失われません。</span></div>
        {capture.error && <p role="alert">保存できませんでした: {String(capture.error)}</p>}
        <div className="form-actions"><HudButton tone="gold" disabled={!rawText.trim() || capture.isPending} type="submit">{capture.isPending ? "保存中…" : captureMode === "deep" ? "保存して分析を確認" : "一言を保存する（+10 XP）"}</HudButton><Link className="hud-button hud-button--cyan" to="/activities/inbox">あとで整理する</Link></div>
      </form>
    </HudPanel>
  </section>;
}
