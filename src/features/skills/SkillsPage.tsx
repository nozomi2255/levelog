import { useQuery } from "@tanstack/react-query";
import { HudBadge, HudButton, HudPanel, ProgressBar } from "../../components/hud";
import { api } from "../../lib/api";

const categoryLabels: Record<string, string> = { thinking: "思考", technical: "技術", communication: "伝える", execution: "実行", interpersonal: "対人" };

export function SkillsPage() {
  const skills = useQuery({ queryKey: ["skills"], queryFn: api.listSkills });
  if (skills.isLoading) return <HudPanel title="スキル"><p className="empty-copy" role="status">スキルの観測を読み込んでいます…</p></HudPanel>;
  if (skills.isError) return <HudPanel title="スキル"><div className="empty-state"><h1>スキルを読み込めませんでした</h1><HudButton onClick={() => void skills.refetch()}>再試行する</HudButton></div></HudPanel>;
  const data = skills.data;
  if (!data) return null;
  const byCategory = data.reduce<Record<string, typeof data>>((groups, skill) => { (groups[skill.category] ??= []).push(skill); return groups; }, {});
  return <section className="feature-page" aria-labelledby="skills-title"><header className="page-heading"><div><p className="hud-kicker">OBSERVED EVIDENCE</p><h1 id="skills-title">スキル観測</h1><p>能力評価ではなく、あなたが確認した証拠の件数です。MVPでは自動昇格しません。</p></div></header><div className="skills-grid" aria-label="スキルの観測一覧">{Object.entries(byCategory).map(([category, items]) => <HudPanel key={category} title={categoryLabels[category] ?? category} className="skills-panel"><ul>{items.map((skill) => <li key={skill.id}><strong>{skill.name}</strong><p className="empty-copy">{skill.code}</p><ProgressBar label={`${skill.name}の証拠数`} value={skill.evidenceCount} max={Math.max(3, skill.evidenceCount)} /><HudBadge>{skill.state === "observing" ? "観測中" : skill.state}</HudBadge></li>)}</ul></HudPanel>)}</div></section>;
}
