import { useTranslations } from "next-intl";
import Link from "next/link";
import type { ReactNode } from "react";

export interface BlogNavProps { slug: string; name: string; current: "posts" | "publications" | "bibliography" | "media" | "links" | "about"; hasPublications?: boolean; actions?: ReactNode }

/** Public blog header: name + secondary nav (Posts · Publications · Bibliography · Media · Links · About). Collapses to a scrollable tab row on mobile. */
export function BlogHeader({ slug, name, current, hasPublications, actions }: BlogNavProps) {
  const t = useTranslations("BlogNav");
  const items: [BlogNavProps["current"], string, string][] = [
    ["posts", t("posts"), `/${slug}`],
    ...(hasPublications ? ([["publications", t("publications"), `/${slug}/pub`]] as [BlogNavProps["current"], string, string][]) : []),
    ["bibliography", t("bibliography"), `/${slug}/bibliografia`],
    ["media", t("media"), `/${slug}/media`],
    ["links", t("links"), `/${slug}/link`],
    ["about", t("about"), `/${slug}/pagina/about`],
  ];
  return (
    <header className="border-b border-border">
      <div className="mx-auto flex h-[60px] max-w-7xl items-center justify-between gap-6 px-5 lg:px-12">
        <div className="flex min-w-0 items-center gap-7">
          <Link href={`/${slug}`} className="truncate font-serif text-[19px] font-semibold text-foreground">{name}</Link>
          <nav className="hidden gap-5 text-sm text-muted md:flex">
            {items.map(([key, label, href]) => <Link key={key} href={href} className={key === current ? "font-medium text-foreground" : "hover:text-foreground"}>{label}</Link>)}
          </nav>
        </div>
        <div className="flex items-center gap-4 text-sm">{actions}</div>
      </div>
      <nav className="flex gap-5 overflow-x-auto px-5 text-sm text-muted md:hidden">
        {items.map(([key, label, href]) => <Link key={key} href={href} className={`whitespace-nowrap pb-2.5 ${key === current ? "-mb-px border-b-2 border-primary font-semibold text-foreground" : ""}`}>{label}</Link>)}
      </nav>
    </header>
  );
}
