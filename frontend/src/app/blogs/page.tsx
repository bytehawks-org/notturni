import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import Link from "next/link";

import { BlogDirectoryGrid } from "@/components/home/BlogDirectory";
import { SiteHeader } from "@/components/SiteHeader";
import { SiteFooter } from "@/components/shell/SiteFooter";
import { FilterChip } from "@/components/ui/Pill";
import { languageName } from "@/lib/languages";
import { getPublicBlogs } from "@/lib/server-api";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("BlogsPage");
  return { title: t("title"), alternates: { canonical: "/blogs" } };
}

const DIRECTORY_LOCALES = ["it", "en", "de", "fr"];

/** Directory pubblica dei blog indicizzabili (mockup 4c). Ricerca e filtro
 * lingua sono applicati qui sull'elenco restituito (max 100): il backend non
 * espone ancora `q`/`locale` su `GET /blogs` (blocco B1). */
export default async function BlogsPage({ searchParams }: { searchParams: Promise<{ locale?: string; q?: string }> }) {
  const { locale, q } = await searchParams;
  const [all, t] = await Promise.all([getPublicBlogs({ limit: 100 }).catch(() => []), getTranslations("BlogsPage")]);
  const needle = q?.trim().toLowerCase();
  const blogs = all.filter(
    (b) =>
      (!locale || b.default_locale === locale) &&
      (!needle || b.title.toLowerCase().includes(needle) || b.slug.includes(needle) || (b.subtitle ?? "").toLowerCase().includes(needle))
  );
  const href = (code: string) => {
    const params = new URLSearchParams();
    if (q) params.set("q", q);
    if (code) params.set("locale", code);
    const qs = params.toString();
    return qs ? `/blogs?${qs}` : "/blogs";
  };
  return (
    <>
      <SiteHeader />
      <main className="mx-auto flex w-full max-w-[1184px] flex-1 flex-col gap-7 px-5 py-10 lg:px-12 lg:py-12">
        <div className="flex flex-col gap-4 md:flex-row md:items-end md:justify-between">
          <div className="flex flex-col gap-1.5">
            <h1 className="font-serif text-3xl font-medium tracking-tight md:text-[40px]">{t("title")}</h1>
            <p className="text-muted md:text-base">{t("subtitle", { count: all.length })}</p>
          </div>
          <form className="flex items-center gap-2 rounded-[10px] border border-border bg-surface px-3.5 py-2 text-sm text-muted md:w-80">
            {locale && <input type="hidden" name="locale" value={locale} />}
            <span aria-hidden="true">⌕</span>
            <input
              name="q"
              defaultValue={q}
              placeholder={t("search")}
              aria-label={t("search")}
              className="w-full bg-transparent text-foreground outline-none placeholder:text-muted"
            />
          </form>
        </div>
        <div className="flex flex-wrap gap-2 border-b border-border pb-3">
          <Link href={href("")} className="no-underline">
            <FilterChip active={!locale}>{t("all")}</FilterChip>
          </Link>
          {DIRECTORY_LOCALES.map((code) => (
            <Link key={code} href={href(code)} className="no-underline">
              <FilterChip active={locale === code}>{languageName(code, code)}</FilterChip>
            </Link>
          ))}
        </div>
        <BlogDirectoryGrid blogs={blogs} />
      </main>
      <SiteFooter />
    </>
  );
}
