import type { ReactNode } from "react";

type Tone = "neutral" | "ok" | "warn" | "info" | "danger" | "primary";

const TONE_CLASSES: Record<Tone, string> = {
  neutral: "text-muted bg-[color-mix(in_srgb,var(--muted)_16%,transparent)]",
  ok: "text-ok bg-ok/15",
  warn: "text-warn bg-[var(--warn-bg)]",
  info: "text-info bg-info/15",
  danger: "text-danger bg-danger/12",
  primary: "text-primary bg-primary/12",
};

export function Pill({ tone = "neutral", children }: { tone?: Tone; children: ReactNode }) {
  return <span className={`inline-flex rounded-full px-2 py-0.5 text-xs font-semibold ${TONE_CLASSES[tone]}`}>{children}</span>;
}

export function TagPill({ children }: { children: ReactNode }) {
  return <span className="inline-flex rounded-full bg-primary/10 px-2.5 py-1 text-[13px] text-primary">#{children}</span>;
}

export function FilterChip({
  active = false,
  children,
  onClick,
}: {
  active?: boolean;
  children: ReactNode;
  onClick?: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-full px-2.5 py-1 text-xs whitespace-nowrap ${
        active ? "bg-foreground text-background" : "border border-border text-muted hover:text-foreground"
      }`}
    >
      {children}
    </button>
  );
}
