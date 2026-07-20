import type { ButtonHTMLAttributes, PropsWithChildren, ReactNode } from "react";

type Tone = "cyan" | "gold" | "green" | "purple";

export function HudPanel({
  title,
  action,
  children,
  className = "",
}: PropsWithChildren<{ title?: string; action?: ReactNode; className?: string }>) {
  return (
    <section className={`hud-panel hud-section ${className}`}>
      {(title || action) && (
        <header className="hud-section__header">
          {title ? <h2 className="hud-section__title">{title}</h2> : <span />}
          {action}
        </header>
      )}
      {children}
    </section>
  );
}

export function HudButton({
  tone = "cyan",
  className = "",
  children,
  ...props
}: PropsWithChildren<ButtonHTMLAttributes<HTMLButtonElement> & { tone?: Tone }>) {
  return <button className={`hud-button hud-button--${tone} ${className}`} {...props}>{children}</button>;
}

export function HudBadge({ children, tone = "cyan" }: PropsWithChildren<{ tone?: Tone }>) {
  return <span className={`hud-badge hud-badge--${tone}`}>{children}</span>;
}

export function ProgressBar({ value, max, label }: { value: number; max: number; label: string }) {
  const percentage = max > 0 ? Math.max(0, Math.min(100, (value / max) * 100)) : 0;
  return (
    <div className="progress" role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={max} aria-valuenow={value}>
      <div className="progress__track"><div className="progress__value" style={{ width: `${percentage}%` }} /></div>
      <span className="progress__label">{value.toLocaleString()} / {max.toLocaleString()}</span>
    </div>
  );
}
