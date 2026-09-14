import type { ReactNode } from "react";

// p-6 di default (il kit lo lascia a ogni chiamante): tutti gli usi
// esistenti di <Card> nel resto dell'app non passano ancora un padding
// esplicito — verrà sovrascritto con className mirate man mano che le
// singole schermate vengono restilizzate nelle fasi successive.
export function Card({
  children,
  className = "",
  accent = false,
}: {
  children: ReactNode;
  className?: string;
  accent?: boolean;
}) {
  return (
    <div
      className={`rounded-xl border bg-surface p-6 ${
        accent ? "border-primary bg-[color-mix(in_srgb,var(--primary)_6%,var(--surface))]" : "border-border"
      } ${className}`}
    >
      {children}
    </div>
  );
}

export function CardTitle({ children }: { children: ReactNode }) {
  return <h2 className="font-serif text-[17px] text-foreground">{children}</h2>;
}

export function SectionLabel({ children }: { children: ReactNode }) {
  return <span className="block font-mono text-[11px] uppercase tracking-[.08em] text-muted">{children}</span>;
}
