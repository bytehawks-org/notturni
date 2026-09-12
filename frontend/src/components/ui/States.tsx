import type { ReactNode } from "react";
import { Button } from "./Button";

export function EmptyState({
  glyph = "❝",
  title,
  body,
  action,
}: {
  glyph?: string;
  title: string;
  body: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center gap-2.5 rounded-xl border border-border bg-surface px-7 py-10 text-center">
      <span className="grid h-14 w-14 place-items-center rounded-full border border-dashed border-muted font-serif text-2xl text-muted">
        {glyph}
      </span>
      <h3 className="mt-1.5 font-serif text-xl">{title}</h3>
      <p className="max-w-[280px] text-sm leading-relaxed text-muted">{body}</p>
      {action && <div className="mt-2">{action}</div>}
    </div>
  );
}

export function ErrorState({
  title,
  body,
  onRetry,
  secondary,
  requestId,
}: {
  title: string;
  body: string;
  onRetry?: () => void;
  secondary?: ReactNode;
  requestId?: string;
}) {
  return (
    <div className="flex flex-col items-center gap-2.5 rounded-xl border border-border bg-surface px-7 py-10 text-center">
      <span className="grid h-14 w-14 place-items-center rounded-full bg-danger/12 text-2xl font-semibold text-danger">!</span>
      <h3 className="mt-1.5 font-serif text-xl">{title}</h3>
      <p className="max-w-[280px] text-sm leading-relaxed text-muted">{body}</p>
      <div className="mt-2 flex gap-2">
        {onRetry && (
          <Button size="sm" onClick={onRetry}>
            Riprova
          </Button>
        )}
        {secondary}
      </div>
      {requestId && <span className="font-mono text-[11px] text-muted">{requestId}</span>}
    </div>
  );
}

/** Riga scheletro che rispecchia una riga di tabella: titolo + meta + pillola. */
export function SkeletonRows({ rows = 4 }: { rows?: number }) {
  const widths = ["78%", "62%", "85%", "54%", "70%"];
  return (
    <div className="flex flex-col gap-4" aria-busy="true">
      {Array.from({ length: rows }).map((_, i) => (
        <div key={i} className="grid grid-cols-[minmax(0,1fr)_70px] items-center gap-3">
          <div className="flex flex-col gap-1.5">
            <span className="block h-3 rounded bg-border" style={{ width: widths[i % widths.length] }} />
            <span className="block h-2.5 w-2/5 rounded bg-border opacity-70" />
          </div>
          <span className="block h-5 rounded-full bg-border" />
        </div>
      ))}
    </div>
  );
}
