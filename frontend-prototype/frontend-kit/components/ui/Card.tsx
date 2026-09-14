import type { ReactNode } from "react";

export function Card({ children, className = "", accent = false }: { children: ReactNode; className?: string; accent?: boolean }) {
  return <div className={`rounded-xl border bg-surface ${accent ? "border-primary bg-[color-mix(in_srgb,var(--primary)_6%,var(--surface))]" : "border-border"} ${className}`}>{children}</div>;
}
export function CardTitle({ children }: { children: ReactNode }) {
  return <h2 className="font-serif text-[17px] text-foreground">{children}</h2>;
}
export function SectionLabel({ children }: { children: ReactNode }) {
  return <span className="block font-mono text-[11px] uppercase tracking-[.08em] text-muted">{children}</span>;
}
