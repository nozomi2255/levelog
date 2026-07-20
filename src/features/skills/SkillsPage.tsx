import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { HudBadge, HudButton, HudPanel } from "../../components/hud";
import { api } from "../../lib/api";
import { CATEGORY_NAMES, FocusThemesSummary, SKILL_ORDER } from "../profile";

export function SkillsPage() {
  const skills = useQuery({ queryKey: ["skills"], queryFn: api.listSkills }); const profile = useQuery({ queryKey: ["user-profile"], queryFn: api.getUserProfile, retry: false });
  if (skills.isLoading) return <HudPanel title="スキル"><p className="empty-copy" role="status">スキルの観測を読み込んでいます…</p></HudPanel>;
  if (skills.isError || !skills.data) return <HudPanel title="スキル"><div className="empty-state"><h1>スキルを読み込めませんでした</h1><HudButton onClick={() => void skills.refetch()}>再試行する</HudButton></div></HudPanel>;
  const focus = new Set(profile.data?.focusSkillIds ?? []); const ordered = [...skills.data].sort((a, b) => SKILL_ORDER.indexOf(a.id) - SKILL_ORDER.indexOf(b.id)); const byCategory = ordered.reduce<Record<string, typeof ordered>>((groups, skill) => { (groups[skill.category] ??= []).push(skill); return groups; }, {});
  return <section className="feature-page" aria-labelledby="skills-title"><header className="page-heading"><div><p className="hud-kicker">OBSERVED EVIDENCE</p><h1 id="skills-title">スキル観測</h1><p>能力の採点ではなく、あなたが確認した証拠を固定15スキルと専門スキルに整理します。</p></div><Link to="/settings">テーマと重点を編集</Link></header><HudPanel title="個人テーマ"><FocusThemesSummary themes={profile.data?.focusThemes ?? []} /></HudPanel><div className="skills-grid" aria-label="固定15スキルの観測一覧">{Object.entries(byCategory).map(([category, items]) => <HudPanel key={category} title={CATEGORY_NAMES[category] ?? category} className="skills-panel"><ul>{items.map((skill) => <li key={skill.id}><div className="skill-heading"><strong>{skill.name}</strong>{focus.has(skill.id) && <HudBadge tone="gold">重点</HudBadge>}</div><p className="evidence-count"><strong>{skill.evidenceCount}</strong> 件の承認済み証拠</p>{skill.specializedSkills.length > 0 && <div><h3>この経験で見えた専門スキル</h3><ul className="specialized-skills">{skill.specializedSkills.map((specialized) => <li key={specialized.name}><span>{specialized.name}</span><small>{specialized.evidenceCount}件</small></li>)}</ul></div>}<HudBadge>観測中</HudBadge></li>)}</ul></HudPanel>)}</div></section>;
}
