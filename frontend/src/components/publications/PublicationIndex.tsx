import { getTranslations } from "next-intl/server";
import Link from "next/link";

import type { Chapter } from "@/lib/types";

/** Indice cronologico di una pubblicazione (mockup 2d, todo/PUBLICATIONS.md). */
export async function PublicationIndex({ chapters, currentPostId }: { chapters: Chapter[]; currentPostId?: string }) {
  const t = await getTranslations("Publication");
  return (
    <ol className="flex flex-col">
      <span className="border-b border-border pb-3.5 font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("index")}</span>
      {chapters.map((c) => {
        const current = c.post_id === currentPostId;
        const meta = !c.is_public ? t("unpublished") : `${t("minutes", { min: c.reading_minutes })}${current ? ` · ${t("continue")}` : ""}`;
        const row = (
          <div className={`grid grid-cols-[40px_minmax(0,1fr)] items-baseline gap-3 border-b border-border py-4 md:grid-cols-[48px_minmax(0,1fr)_140px] md:gap-4 md:py-[18px] ${!c.is_public ? "opacity-50" : ""}`}>
            <span className="font-serif text-xl text-muted md:text-[22px]">{c.n}</span>
            <div className="flex min-w-0 flex-col gap-1">
              <span className="font-serif text-lg leading-snug md:text-[21px]">{c.title}</span>
              <span className="text-xs text-muted md:hidden">{meta}</span>
            </div>
            <span className={`hidden text-right text-[13px] md:block ${current ? "font-semibold text-primary" : "text-muted"}`}>{meta}</span>
          </div>
        );
        return (
          <li key={c.post_id}>
            {c.is_public ? (
              <Link href={c.permalink} className="block text-foreground no-underline hover:text-primary">
                {row}
              </Link>
            ) : (
              row
            )}
          </li>
        );
      })}
    </ol>
  );
}

/** Piè di pagina precedente/successivo dentro un capitolo (mockup 3g). */
export async function ChapterNav({ prev, next, index }: { prev?: Chapter; next?: Chapter; index: { title: string; href: string } }) {
  const t = await getTranslations("Publication");
  return (
    <nav className="mt-10 grid grid-cols-2 gap-3 rounded-xl border border-border bg-surface p-4 text-[13px] md:p-5">
      <div className="flex min-w-0 flex-col gap-0.5 text-muted">
        {prev ? (
          <>
            <span className="text-[11px]">{t("previous")}</span>
            <Link href={prev.permalink} className="truncate font-medium text-foreground no-underline hover:underline">
              {prev.title}
            </Link>
          </>
        ) : (
          <Link href={index.href} className="truncate font-medium text-foreground no-underline hover:underline">
            {t("index")} · {index.title}
          </Link>
        )}
      </div>
      <div className="flex min-w-0 flex-col gap-0.5 text-right text-muted">
        {next ? (
          <>
            <span className="text-[11px]">{t("next")}</span>
            <Link href={next.permalink} className="truncate font-medium text-foreground no-underline hover:underline">
              {next.title}
            </Link>
          </>
        ) : (
          <span className="text-muted">{t("end")}</span>
        )}
      </div>
    </nav>
  );
}

export function ChapterProgress({ current, total }: { current: number; total: number }) {
  return (
    <span className="block h-[3px] w-full bg-border" role="progressbar" aria-valuenow={current} aria-valuemax={total}>
      <span className="block h-full bg-primary" style={{ width: `${Math.round((current / Math.max(1, total)) * 100)}%` }} />
    </span>
  );
}
