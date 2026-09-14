import { useTranslations } from "next-intl";
import Link from "next/link";

export interface Chapter { n: number; title: string; subtitle?: string; href?: string; minutes?: number; read?: boolean; current?: boolean; draft?: boolean }

/** /{blog}/pub/{name} — chronological index (todo/PUBLICATIONS.md). */
export function PublicationIndex({ chapters }: { chapters: Chapter[] }) {
  const t = useTranslations("Publication");
  return (
    <ol className="flex flex-col">
      <span className="border-b border-border pb-3.5 font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("index")}</span>
      {chapters.map((c) => {
        const meta = c.draft ? t("unpublished") : t("minutes", { min: c.minutes ?? 0 }) + (c.read ? ` · ${t("read")}` : c.current ? ` · ${t("continue")}` : "");
        const row = (
          <div className={`grid grid-cols-[40px_minmax(0,1fr)] items-baseline gap-3 border-b border-border py-4 md:grid-cols-[48px_minmax(0,1fr)_120px] md:gap-4 md:py-[18px] ${c.draft ? "opacity-50" : ""}`}>
            <span className="font-serif text-xl text-muted md:text-[22px]">{c.n}</span>
            <div className="flex min-w-0 flex-col gap-1"><span className="font-serif text-lg leading-snug md:text-[21px]">{c.title}</span>{c.subtitle && <span className="text-sm text-muted">{c.subtitle}</span>}<span className="text-xs text-muted md:hidden">{meta}</span></div>
            <span className={`hidden text-right text-[13px] md:block ${c.current ? "font-semibold text-primary" : "text-muted"}`}>{meta}</span>
          </div>
        );
        return <li key={c.n}>{c.href && !c.draft ? <Link href={c.href} className="block text-foreground no-underline hover:text-primary">{row}</Link> : row}</li>;
      })}
    </ol>
  );
}

/** Sticky prev/next footer inside a chapter page; progress bar under the header. */
export function ChapterNav({ prev, next }: { prev?: { title: string; href: string }; next?: { title: string; href: string } }) {
  const t = useTranslations("Publication");
  return (
    <nav className="grid grid-cols-2 gap-3 border-t border-border bg-surface px-5 pb-[max(env(safe-area-inset-bottom),1.25rem)] pt-3 text-[13px] md:rounded-xl md:border md:p-5">
      <div className="flex min-w-0 flex-col gap-0.5 text-muted">{prev && <><span className="text-[11px]">{t("previous")}</span><Link href={prev.href} className="truncate font-medium text-foreground">{prev.title}</Link></>}</div>
      <div className="flex min-w-0 flex-col gap-0.5 text-right text-muted">{next && <><span className="text-[11px]">{t("next")}</span><Link href={next.href} className="truncate font-medium text-foreground">{next.title}</Link></>}</div>
    </nav>
  );
}
export function ChapterProgress({ current, total }: { current: number; total: number }) {
  const t = useTranslations("Publication");
  return <span className="block h-[3px] bg-border" role="progressbar" aria-valuenow={current} aria-valuemax={total}><span className="block h-full bg-primary" style={{ width: `${(current / total) * 100}%` }} /></span>;
}
