export function GrowthCore({ value, observations = 0 }: { value?: number; observations?: number }) {
  const hasValue = typeof value === "number";
  return <section className="growth-core" aria-label="今日のXPと確定した証拠数">
    <div className="growth-core__rings" aria-hidden="true" />
    <p className="hud-kicker">TODAY&apos;S GROWTH</p>
    {hasValue ? <><p className="growth-core__value">{value}<small> XP</small></p><p className="growth-core__observations">確定した証拠 {observations}件</p></> : <p className="growth-core__empty">今日の証拠を<br />記録しましょう</p>}
  </section>;
}
