import { useTranslations } from "next-intl";
import type { ReactNode } from "react";
import type { CSSProperties } from "react";

export type Visibility = "public" | "members" | "private";
/** todo/BLOG.md #2: green / orange / red-black diagonal stripes on the right edge. */
const BAND: Record<Visibility, string> = { public: "var(--vis-public)", members: "var(--vis-members)", private: "var(--vis-private)" };
const LABEL: Record<Visibility, string> = { public: "text-ok", members: "text-[#b8862b]", private: "text-[#8b2e2e]" };

export function VisibilityBand({ visibility }: { visibility: Visibility }) {
  const t = useTranslations("Visibility");
  return <span aria-hidden="true" className="absolute inset-y-0 right-0 w-1.5" style={{ background: BAND[visibility] } as CSSProperties} />;
}
export function VisibilityLabel({ visibility }: { visibility: Visibility }) {
  const t = useTranslations("Visibility");
  return <span className={`text-xs font-medium ${LABEL[visibility]}`}>{t(visibility)}</span>;
}
/** Blog card with the band; use inside a grid. */
export function BlogCard({ name, slug, subtitle, visibility, meta, children }: { name: string; slug: string; subtitle?: string; visibility: Visibility; meta?: ReactNode; children?: ReactNode }) {
  const t = useTranslations("Visibility");
  return (
    <div className="relative flex flex-col gap-2.5 overflow-hidden rounded-xl border border-border bg-surface px-5 py-5">
      <VisibilityBand visibility={visibility} />
      <div className="flex items-center justify-between pr-2"><span className="font-serif text-xl">{name}</span><span className="font-mono text-xs text-muted">{slug}</span></div>
      {subtitle && <span className="text-sm text-muted">{subtitle}</span>}
      <div className="mt-1.5 flex flex-wrap items-center gap-4 text-[13px] text-muted">{meta}<span className="ml-auto"><VisibilityLabel visibility={visibility} /></span></div>
      {children}
    </div>
  );
}
