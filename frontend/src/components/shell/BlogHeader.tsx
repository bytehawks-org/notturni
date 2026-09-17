import { getTranslations } from "next-intl/server";
import Link from "next/link";
import type { ReactNode } from "react";

import { PlatformMark } from "./PlatformMark";

export interface BlogNavProps {
  /** Prefisso dei link interni al blog: `/{slug}` se la pagina è raggiunta
   * path-based, stringa vuota se raggiunta dal proprio sottodominio — vedi
   * `src/lib/blog-path.ts::blogBasePath`. Ogni chiamante lo calcola dal
   * proprio `params`/host, BlogHeader non lo ricava da solo per restare un
   * Server Component senza bisogno di rileggere l'header Host più volte. */
  basePath: string;
  name: string;
  current: "posts" | "publications" | "bibliography" | "media" | "links";
  hasPublications?: boolean;
  actions?: ReactNode;
}

/**
 * Header pubblico del blog (mockup 1a/2a/3f): nome + nav secondaria. Su mobile
 * diventa una riga di tab scorrevole. "Pubblicazioni" appare solo se
 * hasPublications (blocco B9, nessun chiamante lo passa oggi).
 */
export async function BlogHeader({ basePath, name, current, hasPublications, actions }: BlogNavProps) {
  const t = await getTranslations("BlogNav");
  const items: [BlogNavProps["current"], string, string][] = [
    ["posts", t("posts"), basePath || "/"],
    ...(hasPublications ? ([["publications", t("publications"), `${basePath}/pub`]] as [BlogNavProps["current"], string, string][]) : []),
    ["bibliography", t("bibliography"), `${basePath}/bibliografia`],
    ["media", t("media"), `${basePath}/media`],
    ["links", t("links"), `${basePath}/link`],
  ];
  return (
    <header className="border-b border-border">
      <div className="mx-auto flex h-[60px] max-w-7xl items-center justify-between gap-6 px-5 lg:px-12">
        <div className="flex min-w-0 items-center gap-4">
          <PlatformMark />
          <div className="h-5 w-px shrink-0 bg-border" aria-hidden="true" />
          <Link href={basePath || "/"} className="truncate font-serif text-[19px] font-semibold text-foreground no-underline">
            {name}
          </Link>
          <nav className="hidden gap-5 text-sm text-muted md:flex">
            {items.map(([key, label, href]) => (
              <Link
                key={key}
                href={href}
                className={`no-underline ${key === current ? "font-medium text-foreground" : "hover:text-foreground"}`}
              >
                {label}
              </Link>
            ))}
          </nav>
        </div>
        <div className="flex items-center gap-4 text-sm">{actions}</div>
      </div>
      <nav className="flex gap-5 overflow-x-auto px-5 text-sm text-muted md:hidden">
        {items.map(([key, label, href]) => (
          <Link
            key={key}
            href={href}
            className={`whitespace-nowrap pb-2.5 no-underline ${key === current ? "-mb-px border-b-2 border-primary font-semibold text-foreground" : ""}`}
          >
            {label}
          </Link>
        ))}
      </nav>
    </header>
  );
}
