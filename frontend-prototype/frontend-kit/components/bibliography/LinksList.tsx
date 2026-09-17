import { useTranslations } from "next-intl";
import Link from "next/link";

export interface SharedLink { url: string; title: string; post: { title: string; href: string }; date: string }
export interface LinkGroup { host: string; postCount: number; items: SharedLink[] }

/** /{blog}/link — grouped by host. `groupByHost` is a pure helper for the server component. */
export function groupByHost(links: SharedLink[]): LinkGroup[] {
  const t = useTranslations("Links");
  const map = new Map<string, SharedLink[]>();
  for (const l of links) { const h = new URL(l.url).hostname.replace(/^www\./, ""); map.set(h, [...(map.get(h) ?? []), l]); }
  return [...map.entries()].map(([host, items]) => ({ host, items, postCount: new Set(items.map((i) => i.post.href)).size })).sort((a, b) => b.items.length - a.items.length);
}

export function LinksList({ groups }: { groups: LinkGroup[] }) {
  const t = useTranslations("Links");
  return (
    <div className="flex flex-col gap-4 md:gap-8">
      {groups.map((g) => (
        <section key={g.host} className="flex flex-col gap-2.5 border-t border-border pt-3.5 md:grid md:grid-cols-[220px_minmax(0,1fr)] md:gap-7 md:pt-6">
          <div className="flex items-baseline justify-between md:flex-col md:items-start md:gap-1"><h2 className="font-serif text-[17px] md:text-[19px]">{g.host}</h2><span className="text-xs text-muted md:text-[13px]">{t("count", { count: g.items.length })}{g.postCount > 1 && ` · ${t("posts", { count: g.postCount })}`}</span></div>
          <ul className="flex flex-col gap-2.5 md:gap-3.5">
            {g.items.map((l) => (
              <li key={l.url} className="flex min-w-0 flex-col gap-0.5 md:grid md:grid-cols-[minmax(0,1fr)_150px] md:gap-4">
                <div className="flex min-w-0 flex-col gap-0.5"><a href={l.url} rel="noopener noreferrer" className="text-[15px] font-medium text-foreground md:text-base">{l.title}</a><span className="hidden truncate font-mono text-[13px] text-muted md:block">{l.url}</span></div>
                <span className="text-xs text-muted md:text-right md:text-[13px] md:leading-snug">{t("in")} <Link href={l.post.href}>{l.post.title}</Link><br className="hidden md:block" /> · {l.date}</span>
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}
