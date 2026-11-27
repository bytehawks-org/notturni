import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { notFound } from "next/navigation";

import { BlogPageShell } from "@/components/blog/BlogPageShell";
import { BlogStateNotice, blogIsOffline } from "@/components/blog/BlogStateNotice";
import { FeedPostCard } from "@/components/FeedPostCard";
import { SearchForm } from "@/components/search/SearchForm";
import { SearchPager } from "@/components/search/SearchPager";
import { BlogHeader } from "@/components/shell/BlogHeader";
import { currentBlogLinks } from "@/lib/blog-links";
import { normalizeQuery } from "@/lib/search";
import { SEARCH_PAGE_SIZE, parseSearchPage, searchPageHref } from "@/lib/search-pagination";
import { fetchSearchResultsPage } from "@/lib/search-results";
import { getPublicBlog, getPublicBlogConfig, getPublicPublications, searchBlogPosts } from "@/lib/server-api";

interface PageParams {
  blogSlug: string;
}

export async function generateMetadata({ params }: { params: Promise<PageParams> }): Promise<Metadata> {
  const { blogSlug } = await params;
  const [blog, t] = await Promise.all([getPublicBlog(blogSlug), getTranslations("BlogSearchPage")]);
  if (!blog) return { title: t("title", { blog: blogSlug }) };
  return { title: t("title", { blog: blog.title }), robots: { index: false, follow: false } };
}

/** Ricerca ristretta al singolo blog (sottodominio incluso — CLAUDE.md #5):
 * a differenza di `/search` (ricerca globale sul portale), interroga solo
 * `GET /blogs/{slug}/search`, mai gli altri blog. */
export default async function BlogSearchPage({
  params,
  searchParams,
}: {
  params: Promise<PageParams>;
  searchParams: Promise<{ q?: string; page?: string }>;
}) {
  const [{ blogSlug }, { q, page: rawPage }] = await Promise.all([params, searchParams]);
  const query = normalizeQuery(q ?? "");
  const page = parseSearchPage(rawPage);
  const [blog, config, t, links] = await Promise.all([
    getPublicBlog(blogSlug),
    getPublicBlogConfig(blogSlug),
    getTranslations("BlogSearchPage"),
    currentBlogLinks(blogSlug),
  ]);
  if (!blog) notFound();
  const hasPublications = (await getPublicPublications(blogSlug).catch(() => [])).length > 0;
  if (blogIsOffline(blog)) {
    return (
      <BlogPageShell config={config}>
        <BlogHeader blogSlug={blogSlug} basePath={links.basePath} homeHref={links.homeHref} name={blog.title} hasPublications={hasPublications} current="posts" />
        <BlogStateNotice blog={blog} />
      </BlogPageShell>
    );
  }
  const results = query
    ? await fetchSearchResultsPage((options) => searchBlogPosts(blogSlug, query, options), page)
    : { items: [], hasNext: false, rateLimited: false };
  const { items: posts, hasNext, rateLimited } = results;
  const showPager = query !== "" && !rateLimited && (page > 1 || hasNext);

  return (
    <BlogPageShell config={config}>
      <BlogHeader blogSlug={blogSlug} basePath={links.basePath} homeHref={links.homeHref} name={blog.title} hasPublications={hasPublications} current="posts" />
      <main className="mx-auto w-full max-w-[1184px] flex-1 px-5 py-10 lg:px-12 lg:py-14">
        <div className="flex flex-col gap-4">
          <h1 className="font-serif text-[34px] font-medium leading-[1.12] tracking-tight md:text-[40px]">
            {t("title", { blog: blog.title })}
          </h1>
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
            <p className="mt-6 text-[13px] text-muted">
              {t("resultsRange", {
                from: (page - 1) * SEARCH_PAGE_SIZE + 1,
                to: (page - 1) * SEARCH_PAGE_SIZE + posts.length,
                query,
              })}
            </p>
            <div className="flex flex-col">
              {posts.map((post) => (
                <FeedPostCard key={post.id} post={post} showBlog={false} resolvePermalink={links.fromPermalink} />
              ))}
            </div>
          </>
        )}
        {showPager && (
          <div className="mt-8">
            <SearchPager
              page={page}
              hasNext={hasNext}
              hrefFor={(n) => searchPageHref(links.basePath, query, n)}
              labels={{
                previous: t("previous"),
                next: t("next"),
                page: t("page", { page }),
                nav: t("pageNav"),
              }}
            />
          </div>
        )}
      </main>
    </BlogPageShell>
  );
}
