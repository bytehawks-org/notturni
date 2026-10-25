import type { Metadata } from "next";
import { getLocale, getTranslations } from "next-intl/server";
import { notFound } from "next/navigation";

import { SiteHeader } from "@/components/SiteHeader";
import { SiteFooter } from "@/components/shell/SiteFooter";
import { excerpt, renderMarkdown } from "@/lib/markdown";
import Link from "next/link";

import { LOCALE_NAMES, isLocale } from "@/i18n/config";
import { getPublicPlatformPageForLocales } from "@/lib/server-api";

interface PageParams {
  slug: string;
}

/** Lingua della pagina: quella richiesta con `?locale=`, poi quella dell'interfaccia del visitatore, poi
 * l'italiano e l'inglese; se la pagina non esiste in nessuna di queste, la prima traduzione pubblicata. */
async function resolve(slug: string, requested?: string) {
  const ui = await getLocale();
  return getPublicPlatformPageForLocales(slug, [requested ?? "", ui, "it", "en"]);
}

/** Pagina statica pubblica del sito principale (Chi siamo, Contatti,
 * Privacy, ...), gestita da Amministratore/Super Admin (CLAUDE.md #1).
 * Permalink dedicato `/p/{slug}`: un solo carattere, già sotto la lunghezza
 * minima per gli slug dei blog, quindi nessun blog può mai collidere con
 * questo prefisso (vedi backend/app/domain/blog_rules.py). */
export async function generateMetadata({
  params,
  searchParams,
}: {
  params: Promise<PageParams>;
  searchParams: Promise<{ locale?: string }>;
}): Promise<Metadata> {
  const { slug } = await params;
  const { locale } = await searchParams;
  const found = await resolve(slug, locale);
  if (!found) return {};
  const { page } = found;
  return {
    title: page.title,
    description: excerpt(page.content),
    alternates: { canonical: `/p/${slug}` },
  };
}

export default async function PublicPlatformPagePage({
  params,
  searchParams,
}: {
  params: Promise<PageParams>;
  searchParams: Promise<{ locale?: string }>;
}) {
  const { slug } = await params;
  const { locale } = await searchParams;
  const found = await resolve(slug, locale);
  if (!found) notFound();
  const { page, locales } = found;

  const tPost = await getTranslations("Post");
  const tPage = await getTranslations("StaticPage");
  const html = await renderMarkdown(page.content, {
    expandImageLabel: tPost("expandImage"),
    copyCodeLabel: tPost("copyCode"),
    copiedCodeLabel: tPost("copiedCode"),
  });

  return (
    <div className="flex flex-1 flex-col">
      <SiteHeader />
      <main className="mx-auto w-full max-w-2xl flex-1 px-6 py-12">
        {locales.length > 1 && (
          <nav aria-label={tPage("languages")} className="flex flex-wrap gap-2 text-[13px]">
            {locales.map((code) => (
              <Link
                key={code}
                href={`/p/${slug}?locale=${code}`}
                aria-current={code === page.locale ? "true" : undefined}
                className={`rounded-full px-3 py-1 no-underline ${code === page.locale ? "bg-foreground text-background" : "border border-border text-muted hover:text-foreground"}`}
              >
                {isLocale(code) ? LOCALE_NAMES[code] : code.toUpperCase()}
              </Link>
            ))}
          </nav>
        )}
        <h1 className="mt-6 font-serif text-5xl font-semibold leading-tight text-foreground">{page.title}</h1>

        <div className="notturni-prose mt-10 text-lg leading-relaxed" dangerouslySetInnerHTML={{ __html: html }} />
      </main>
      <SiteFooter />
    </div>
  );
}
