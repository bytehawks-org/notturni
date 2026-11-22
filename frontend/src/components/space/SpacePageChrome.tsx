import Link from "next/link";

import type { OutlineNode } from "@/lib/types";

/** Percorso dalla radice dello Spazio alla pagina corrente. */
export function Breadcrumbs({
  spaceName,
  spaceHref,
  trail,
  hrefFor,
  label,
}: {
  spaceName: string;
  spaceHref: string;
  trail: OutlineNode[];
  hrefFor: (node: OutlineNode) => string;
  label: string;
}) {
  return (
    <nav aria-label={label} className="mb-5 flex flex-wrap items-center gap-x-2 gap-y-1 text-[13px] text-muted">
      <Link href={spaceHref} className="no-underline hover:text-foreground">
        {spaceName}
      </Link>
      {trail.map((n) => (
        <span key={n.id} className="flex items-center gap-2">
          <span aria-hidden="true">/</span>
          <Link href={hrefFor(n)} className="no-underline hover:text-foreground">
            {n.title}
          </Link>
        </span>
      ))}
    </nav>
  );
}

/** Pagina precedente e successiva in ordine di lettura (modalità docs). */
export function PrevNext({
  prev,
  next,
  hrefFor,
  prevLabel,
  nextLabel,
}: {
  prev: OutlineNode | null;
  next: OutlineNode | null;
  hrefFor: (node: OutlineNode) => string;
  prevLabel: string;
  nextLabel: string;
}) {
  if (!prev && !next) return null;
  return (
    <nav className="mt-12 grid gap-3 border-t border-border pt-6 sm:grid-cols-2">
      {prev ? (
        <Link href={hrefFor(prev)} className="rounded-xl border border-border bg-surface px-4 py-3 no-underline transition hover:border-primary/50">
          <span className="block text-[11px] uppercase tracking-[.08em] text-muted">← {prevLabel}</span>
          <span className="font-medium text-foreground">{prev.title}</span>
        </Link>
      ) : (
        <span />
      )}
      {next && (
        <Link href={hrefFor(next)} className="rounded-xl border border-border bg-surface px-4 py-3 text-right no-underline transition hover:border-primary/50">
          <span className="block text-[11px] uppercase tracking-[.08em] text-muted">{nextLabel} →</span>
          <span className="font-medium text-foreground">{next.title}</span>
        </Link>
      )}
    </nav>
  );
}
