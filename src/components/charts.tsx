export interface RadarDatum { label: string; value: number; max?: number }

export function RadarChart({ data }: { data: RadarDatum[] }) {
  if (data.length === 0) return <EmptyChart label="能力を記録すると、ここに傾向が表示されます。" />;
  const size = 220;
  const center = size / 2;
  const radius = 70;
  const point = (value: number, index: number) => {
    const angle = (Math.PI * 2 * index) / data.length - Math.PI / 2;
    const datum = data[index]!;
    return `${center + Math.cos(angle) * radius * Math.min(value / (datum.max ?? 100), 1)},${center + Math.sin(angle) * radius * Math.min(value / (datum.max ?? 100), 1)}`;
  };
  const frame = data.map((_, index) => point(data[index]!.max ?? 100, index)).join(" ");
  const values = data.map((datum, index) => point(datum.value, index)).join(" ");
  return <svg className="hud-chart" viewBox={`0 0 ${size} ${size}`} role="img" aria-label="能力レーダーチャート"><title>能力レーダーチャート</title><polygon points={frame} className="radar-frame" /><polygon points={values} className="radar-values" />{data.map((datum, index) => { const angle = (Math.PI * 2 * index) / data.length - Math.PI / 2; return <text key={datum.label} x={center + Math.cos(angle) * 96} y={center + Math.sin(angle) * 96} className="chart-label" textAnchor="middle">{datum.label}</text>; })}</svg>;
}

export interface LineDatum { date: string; xp: number }

export function WeeklyLineChart({ points }: { points: LineDatum[] }) {
  if (points.length === 0) return <EmptyChart label="活動を承認すると、週間推移が表示されます。" />;
  const width = 460; const height = 180; const pad = 28;
  const max = Math.max(...points.map((point) => point.xp), 1);
  const coords = points.map((point, index) => `${pad + (index * (width - pad * 2)) / Math.max(points.length - 1, 1)},${height - pad - (point.xp / max) * (height - pad * 2)}`);
  return <svg className="hud-chart hud-chart--line" viewBox={`0 0 ${width} ${height}`} role="img" aria-label="直近7日間の成長推移"><title>直近7日間の成長推移</title><line x1={pad} y1={height - pad} x2={width - pad} y2={height - pad} className="line-grid" /><polyline points={coords.join(" ")} className="line-values" />{coords.map((coord, index) => { const [x, y] = coord.split(","); const datum = points[index]!; return <g key={datum.date}><circle cx={x} cy={y} r="3" className="line-dot" /><text x={x} y={height - 8} className="chart-label" textAnchor="middle">{datum.date}</text></g>; })}</svg>;
}

function EmptyChart({ label }: { label: string }) { return <p className="chart-empty">{label}</p>; }
