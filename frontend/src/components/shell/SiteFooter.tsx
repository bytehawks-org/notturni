import Link from "next/link";
import { getLocale, getTranslations } from "next-intl/server";

import { renderSimpleMarkdown } from "@/lib/markdown";
import { getPlatformFooter, getPublicLegalLinks } from "@/lib/server-api";
import type { BlogConfig } from "@/lib/types";

interface SiteFooterProps {
  /** Presente solo quando il footer compare dentro un blog: le colonne 1/2
   * possono essere sovrascritte da `blogConfig.footer` (mai la 3 né la barra
   * inferiore, sempre e solo di piattaforma) — vedi
   * `crates/noct-blogs/src/rules.rs`. */
  blogConfig?: BlogConfig | null;
}

/** Footer mostrato su ogni pagina pubblica, di piattaforma e di ogni blog
 * (richiesta esplicita): 3 colonne di Markdown libero (immagini/link
 * inclusi) più una barra inferiore sempre e solo di piattaforma. Server
 * Component asincrono: non importabile da un Client Component (vedi
 * `app/u/[username]/layout.tsx`, che lo rende al posto della pagina). Assente
 * ovunque non ci sia nulla di configurato, invece di un footer vuoto. */
export async function SiteFooter({ blogConfig }: SiteFooterProps = {}) {
  const locale = await getLocale();
  const [footer, legalLinks, t] = await Promise.all([getPlatformFooter(), getPublicLegalLinks(locale), getTranslations("Footer")]);
  const override = blogConfig?.footer;
  const columns = [override?.column1 || footer.column1, override?.column2 || footer.column2, footer.column3];
  const hasColumns = columns.some((c) => c && c.trim());
  const bottomBar = footer.bottom_bar;
  const hasBottomBar = Boolean(bottomBar && bottomBar.trim());

  if (!hasColumns && !hasBottomBar && legalLinks.length === 0) return null;

  return (
    <footer className="border-t border-border">
      {hasColumns && (
        <div className="mx-auto grid w-full max-w-[1184px] gap-8 px-5 py-10 text-[13px] text-muted sm:grid-cols-3 lg:px-12">
          {columns.map((markdown, i) =>
            markdown && markdown.trim() ? (
              <div key={i} className="notturni-prose" dangerouslySetInnerHTML={{ __html: renderSimpleMarkdown(markdown) }} />
            ) : (
              <div key={i} />
            )
          )}
        </div>
      )}
      {legalLinks.length > 0 && (
        <nav
          aria-label={t("legal")}
          className="mx-auto flex w-full max-w-[1184px] flex-wrap gap-x-5 gap-y-1 border-t border-border px-5 py-3 text-[12px] text-muted lg:px-12"
        >
          {legalLinks.map((l) => (
            <Link key={l.doc} href={`/p/${l.slug}`} className="no-underline hover:text-foreground">
              {l.title}
            </Link>
          ))}
        </nav>
      )}
      {hasBottomBar && (
        <div className="border-t border-border">
          <div
            className="notturni-prose mx-auto w-full max-w-[1184px] px-5 py-3 text-[12px] text-muted lg:px-12"
            dangerouslySetInnerHTML={{ __html: renderSimpleMarkdown(bottomBar as string) }}
          />
        </div>
      )}
    </footer>
  );
}
