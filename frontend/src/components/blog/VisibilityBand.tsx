"use client";

import { useTranslations } from "next-intl";
import type { ReactNode } from "react";

import type { BlogVisibility } from "@/lib/types";

/** todo/BLOG.md #2: banda verde / arancio / strisce rosso-nero sul bordo
 * destro della card (variabili `--vis-*` in globals.css). */
const BAND: Record<BlogVisibility, string> = {
  public: "var(--vis-public)",
  members: "var(--vis-members)",
  private: "var(--vis-private)",
};
const LABEL: Record<BlogVisibility, string> = { public: "text-ok", members: "text-[#b8862b]", private: "text-[#8b2e2e]" };

export function VisibilityBand({ visibility }: { visibility: BlogVisibility }) {
  const t = useTranslations("Visibility");
  return <span aria-hidden="true" title={t(visibility)} className="absolute inset-y-0 right-0 w-1.5" style={{ background: BAND[visibility] }} />;
}

export function VisibilityLabel({ visibility }: { visibility: BlogVisibility }) {
  const t = useTranslations("Visibility");
  return <span className={`text-xs font-medium ${LABEL[visibility]}`}>{t(visibility)}</span>;
}

/** Card di un blog con la banda (mockup 1e/1f/5c), da usare dentro una griglia. */
export function BlogCard({
  name,
  slug,
  subtitle,
  visibility,
  meta,
  children,
}: {
  name: string;
  slug: string;
  subtitle?: string | null;
  visibility: BlogVisibility;
  meta?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="relative flex h-full flex-col gap-2.5 overflow-hidden rounded-xl border border-border bg-surface px-5 py-5 transition hover:border-primary">
      <VisibilityBand visibility={visibility} />
      <div className="flex items-center justify-between gap-3 pr-2">
        <span className="truncate font-serif text-xl">{name}</span>
        <span className="shrink-0 font-mono text-xs text-muted">{slug}</span>
      </div>
      {subtitle && <span className="text-sm text-muted">{subtitle}</span>}
      <div className="mt-auto flex flex-wrap items-center gap-4 pt-1 text-[13px] text-muted">
        {meta}
        <span className="ml-auto">
          <VisibilityLabel visibility={visibility} />
        </span>
      </div>
      {children}
    </div>
  );
}
