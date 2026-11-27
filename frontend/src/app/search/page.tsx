import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";

import { FeedPostCard } from "@/components/FeedPostCard";
import { SiteHeader } from "@/components/SiteHeader";
import { SearchForm } from "@/components/search/SearchForm";
import { SearchPager } from "@/components/search/SearchPager";
import { SiteFooter } from "@/components/shell/SiteFooter";
import { SEARCH_PAGE_SIZE, parseSearchPage, searchPageHref } from "@/lib/search-pagination";
import { normalizeQuery } from "@/lib/search";
import { searchPosts } from "@/lib/server-api";
import { fetchSearchResultsPage } from "@/lib/search-results";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("SearchPage");
  return { title: t("title"), robots: { index: false, follow: false } };
}

/** Ricerca globale sul portale (CLAUDE.md #5): titolo/contenuto tra i post
 * di ogni blog pubblico, `GET /api/v1/search/posts`. La versione per singolo
 * blog vive su `/{blogSlug}/search` (sottodominio incluso, vedi `proxy.ts` —
 * "search" non è tra i `PLATFORM_ONLY_PATHS`, quindi sul sottodominio del
 * blog questa route resta quella scoped, non questa). Non indicizzata
 * (`noindex`): pagina di risultati, non contenuto originale. Paginata con
 * `?page=` (20 per pagina), senza JavaScript lato client. */
export default async function SearchPage({
  searchParams,
}: {
  searchParams: Promise<{ q?: string; page?: string }>;
}) {
  const params = await searchParams;
  const query = normalizeQuery(params.q ?? "");
  const page = parseSearchPage(params.page);
  const [results, t] = await Promise.all([
    query
      ? fetchSearchResultsPage((options) => searchPosts(query, options), page)
      : Promise.resolve({ items: [], hasNext: false, rateLimited: false }),
    getTranslations("SearchPage"),
  ]);
  const { items: posts, hasNext, rateLimited } = results;
  const showPager = query !== "" && !rateLimited && (page > 1 || hasNext);

  return (
    <>
      <SiteHeader />
      <main className="mx-auto flex w-full max-w-[1184px] flex-1 flex-col gap-7 px-5 py-10 lg:px-12 lg:py-12">
        <div className="flex flex-col gap-4">
          <h1 className="font-serif text-3xl font-medium tracking-tight md:text-[40px]">{t("title")}</h1>
          <SearchForm defaultValue={query} placeholder={t("placeholder")} submitLabel={t("searchButton")} />
        </div>
        {!query ? (
          <p className="py-10 text-center text-sm text-muted">{t("noQuery")}</p>
        ) : rateLimited ? (
          <p role="alert" className="py-10 text-center text-sm text-muted">{t("rateLimited")}</p>
        ) : posts.length === 0 ? (
          <p className="py-10 text-center text-sm text-muted">{t("noResults", { query })}</p>
        ) : (
          <>
            <p className="text-[13px] text-muted">
              {t("resultsRange", {
                from: (page - 1) * SEARCH_PAGE_SIZE + 1,
                to: (page - 1) * SEARCH_PAGE_SIZE + posts.length,
                query,
              })}
            </p>
            <div className="flex flex-col">
              {posts.map((post) => (
                <FeedPostCard key={post.id} post={post} showBlog />
              ))}
            </div>
          </>
        )}
        {showPager && (
          <SearchPager
            page={page}
            hasNext={hasNext}
            hrefFor={(n) => searchPageHref("", query, n)}
            labels={{
              previous: t("previous"),
              next: t("next"),
              page: t("page", { page }),
              nav: t("pageNav"),
            }}
          />
        )}
      </main>
      <SiteFooter />
    </>
  );
}
