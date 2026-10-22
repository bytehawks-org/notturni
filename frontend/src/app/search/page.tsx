import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";

import { FeedPostCard } from "@/components/FeedPostCard";
import { SiteHeader } from "@/components/SiteHeader";
import { SiteFooter } from "@/components/shell/SiteFooter";
import { searchPosts } from "@/lib/server-api";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("SearchPage");
  return { title: t("title"), robots: { index: false, follow: false } };
}

/** Ricerca globale sul portale (CLAUDE.md #5): titolo/contenuto tra i post
 * di ogni blog pubblico, `GET /api/v1/search/posts`. La versione per singolo
 * blog vive su `/{blogSlug}/search` (sottodominio incluso, vedi `proxy.ts` —
 * "search" non è tra i `PLATFORM_ONLY_PATHS`, quindi sul sottodominio del
 * blog questa route resta quella scoped, non questa). Non indicizzata
 * (`noindex`): pagina di risultati, non contenuto originale. */
export default async function SearchPage({
  searchParams,
}: {
  searchParams: Promise<{ q?: string }>;
}) {
  const { q } = await searchParams;
  const query = q?.trim() ?? "";
  const [posts, t] = await Promise.all([
    query ? searchPosts(query, { limit: 30 }) : Promise.resolve([]),
    getTranslations("SearchPage"),
  ]);

  return (
    <>
      <SiteHeader />
      <main className="mx-auto flex w-full max-w-[1184px] flex-1 flex-col gap-7 px-5 py-10 lg:px-12 lg:py-12">
        <div className="flex flex-col gap-4">
          <h1 className="font-serif text-3xl font-medium tracking-tight md:text-[40px]">{t("title")}</h1>
          <form className="flex items-center gap-2 rounded-[10px] border border-border bg-surface px-3.5 py-2 text-sm text-muted md:w-96">
            <span aria-hidden="true">⌕</span>
            <input
              name="q"
              defaultValue={query}
              placeholder={t("placeholder")}
              aria-label={t("placeholder")}
              autoFocus
              className="w-full bg-transparent text-foreground outline-none placeholder:text-muted"
            />
          </form>
        </div>
        {!query ? (
          <p className="py-10 text-center text-sm text-muted">{t("noQuery")}</p>
        ) : posts.length === 0 ? (
          <p className="py-10 text-center text-sm text-muted">{t("noResults", { query })}</p>
        ) : (
          <>
            <p className="text-[13px] text-muted">{t("resultsCount", { count: posts.length, query })}</p>
            <div className="flex flex-col">
              {posts.map((post) => (
                <FeedPostCard key={post.id} post={post} showBlog />
              ))}
            </div>
          </>
        )}
      </main>
      <SiteFooter />
    </>
  );
}
