import { useTranslations } from "next-intl";
import Link from "next/link";

export interface BibEntry { id: string; text: string; kind: "book" | "article" | "web" | "note"; count: number; firstCited: string; posts: { title: string; href: string }[] }

/** /{blog}/bibliografia — deduplicated notes with citing posts. Mobile collapses to 2 columns. */
export function BibliographyList({ entries }: { entries: BibEntry[] }) {
  const t = useTranslations("Bibliography");
  return (
    <ol className="flex flex-col">
      {entries.map((e, i) => (
        <li key={e.id} className="grid grid-cols-[30px_minmax(0,1fr)] gap-2.5 border-b border-border py-3.5 md:grid-cols-[44px_minmax(0,1fr)_200px] md:gap-5 md:py-5">
          <span className="pt-0.5 font-serif text-[15px] text-muted">{String(i + 1).padStart(2, "0")}</span>
          <div className="flex min-w-0 flex-col gap-2">
            <span className="text-[15px] leading-[1.45] md:text-[17px]">{e.text}</span>
            <div className="flex flex-wrap items-center gap-1.5 text-[13px] text-muted">
              <span className="rounded border border-border px-1.5 py-0.5 font-mono text-[11px] uppercase tracking-[.06em]">{t(`kind.${e.kind}`)}</span>
              <span className="hidden md:inline">{t("citedIn")}</span>
              {e.posts.map((p) => <Link key={p.href} href={p.href} className="hidden text-[13px] md:inline">{p.title}</Link>)}
              <span className="md:hidden">· {t("citations", { count: e.count })}</span>
            </div>
          </div>
          <div className="hidden flex-col items-end gap-1 text-right text-[13px] text-muted md:flex"><span className="font-semibold text-foreground">{t("citations", { count: e.count })}</span><span>{t("firstCited", { date: e.firstCited })}</span></div>
        </li>
      ))}
    </ol>
  );
}
