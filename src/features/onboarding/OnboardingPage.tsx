import { useMutation } from "@tanstack/react-query";
import { useState, type FormEvent } from "react";
import { HudBadge, HudButton, HudPanel } from "../../components/hud";
import { api } from "../../lib/api";

const SKILL_GROUPS = [
  ["思考力", [["thinking.information_structuring", "情報整理"], ["thinking.problem_decomposition", "問題分解"], ["thinking.hypothesis_testing", "仮説検証"]]],
  ["技術力", [["technical.technical_learning", "技術学習"], ["technical.system_design", "システム設計"], ["technical.validation", "検証"]]],
  ["伝える力", [["communication.clarification", "確認質問"], ["communication.explanation", "説明"], ["communication.documentation", "文章化"]]],
  ["実行力", [["execution.prioritization", "優先順位"], ["execution.planning", "計画"], ["execution.follow_through", "やり切る"]]],
  ["対人力", [["interpersonal.listening", "傾聴"], ["interpersonal.alignment", "認識合わせ"], ["interpersonal.feedback", "フィードバック"]]],
] as const;

export function OnboardingPage({ onSaved }: { onSaved?: () => void }) {
  const [role, setRole] = useState("");
  const [skills, setSkills] = useState<string[]>([]);
  const [minutes, setMinutes] = useState(120);
  const [excluded, setExcluded] = useState("");
  const [codexPath, setCodexPath] = useState("");
  const connection = useMutation({ mutationFn: () => api.testCodexConnection(codexPath.trim()) });
  const save = useMutation({ mutationFn: () => api.saveOnboarding({ role, focusSkillIds: skills, weeklyMinutes: minutes, excludedQuestPatterns: excluded, codexPath: codexPath.trim() }), onSuccess: onSaved });
  const toggleSkill = (id: string) => setSkills((current) => current.includes(id) ? current.filter((skill) => skill !== id) : current.length < 3 ? [...current, id] : current);
  const submit = (event: FormEvent) => { event.preventDefault(); save.mutate(); };

  return <div className="onboarding-card">
    <header className="onboarding-hero"><HudBadge tone="purple">SYSTEM INITIALIZATION</HudBadge><p className="hud-kicker">LEVEL·LOG / LOCAL GROWTH SYSTEM</p><h1 id="onboarding-title">成長プロフィールを設定</h1><p>日々の証拠を、あなたの状況に合った小さな挑戦へつなげます。入力内容はローカルに保存されます。</p></header>
    <form className="feature-form" onSubmit={submit} aria-labelledby="onboarding-title">
      <HudPanel title="01 · 現在地">
        <label className="field">現在の役割<input required value={role} onChange={(event) => setRole(event.target.value)} placeholder="例：プロダクトエンジニア" /></label>
        <label className="field">週に使える時間（分）<input required min="1" max="10080" type="number" value={minutes} onChange={(event) => setMinutes(Number(event.target.value))} /></label>
        <label className="field">避けたいクエスト（任意）<textarea value={excluded} onChange={(event) => setExcluded(event.target.value)} placeholder="例：他者への連絡を伴うもの、30分を超えるもの" /></label>
      </HudPanel>
      <HudPanel title="02 · 重点スキル（1〜3個）">
        <p className="form-help">現在伸ばしたい領域を選択してください。AIの観測は、この選択だけに限定されません。</p>
        <div className="skill-picker">{SKILL_GROUPS.map(([category, items]) => <fieldset key={category}><legend>{category}</legend>{items.map(([id, label]) => <label className="check-card" key={id}><input type="checkbox" checked={skills.includes(id)} disabled={!skills.includes(id) && skills.length >= 3} onChange={() => toggleSkill(id)} /><span>{label}</span></label>)}</fieldset>)}</div>
        <p className="selection-count" role="status">{skills.length} / 3 選択中</p>
      </HudPanel>
      <HudPanel title="03 · Codex接続">
        <label className="field">Codex CLIの絶対パス<input required placeholder="/opt/homebrew/bin/codex" value={codexPath} onChange={(event) => setCodexPath(event.target.value)} /></label>
        <p className="form-help">AI分析を開始する前に送信JSONを確認・編集できます。接続テストは保存前でも実行できます。</p>
        <div className="form-actions"><HudButton type="button" onClick={() => connection.mutate()} disabled={!codexPath.trim() || connection.isPending}>{connection.isPending ? "接続を確認中…" : "接続テスト"}</HudButton></div>
        {connection.data && <p role="status"><HudBadge tone={connection.data.available && connection.data.authenticated ? "green" : "gold"}>{connection.data.available && connection.data.authenticated ? "READY" : "CHECK"}</HudBadge> {connection.data.message}</p>}
        {connection.error && <p role="alert">{String(connection.error)}</p>}
      </HudPanel>
      {save.error && <p role="alert">{String(save.error)}</p>}
      <div className="onboarding-submit"><HudButton tone="gold" type="submit" disabled={skills.length === 0 || save.isPending}>{save.isPending ? "保存中…" : "保存してLevelogを開始"}</HudButton></div>
    </form>
  </div>;
}
