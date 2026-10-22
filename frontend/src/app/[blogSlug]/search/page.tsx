import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { notFound } from "next/navigation";

import { BlogPageShell } from "@/components/blog/BlogPageShell";
import { BlogStateNotice, blogIsOffline } from "@/components/blog/BlogStateNotice";
import { FeedPostCard } from "@/components/FeedPostCard";
import { BlogHeader } from "@/components/shell/BlogHeader";
import { blogLinks } from "@/lib/blog-path";
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
  searchParams: Promise<{ q?: string }>;
}) {
  const [{ blogSlug }, { q }] = await Promise.all([params, searchParams]);
  const query = q?.trim() ?? "";
  const [blog, config, t, links] = await Promise.all([
    getPublicBlog(blogSlug),
    getPublicBlogConfig(blogSlug),
    getTranslations("BlogSearchPage"),
    blogLinks(blogSlug),
  ]);
  if (!blog) notFound();
  const hasPublications = (await getPublicPublications(blogSlug).catch(() => [])).length > 0;
  if (blogIsOffline(blog)) {
    return (
      <BlogPageShell config={config}>
        <BlogHeader basePath={links.basePath} name={blog.title} hasPublications={hasPublications} current="posts" />
        <BlogStateNotice blog={blog} />
      </BlogPageShell>
    );
  }
  const posts = query ? await searchBlogPosts(blogSlug, query, { limit: 30 }) : [];

  return (
    <BlogPageShell config={config}>
      <BlogHeader basePath={links.basePath} name={blog.title} hasPublications={hasPublications} current="posts" />
      <main className="mx-auto w-full max-w-[1184px] flex-1 px-5 py-10 lg:px-12 lg:py-14">
        <div className="flex flex-col gap-4">
          <h1 className="font-serif text-[34px] font-medium leading-[1.12] tracking-tight md:text-[40px]">
            {t("title", { blog: blog.title })}
          </h1>
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
        ) : !posts || posts.length === 0 ? (
          <p className="py-10 text-center text-sm text-muted">{t("noResults", { query })}</p>
        ) : (
          <>
            <p className="mt-6 text-[13px] text-muted">{t("resultsCount", { count: posts.length, query })}</p>
            <div className="flex flex-col">
              {posts.map((post) => (
                <FeedPostCard key={post.id} post={post} showBlog={false} resolvePermalink={links.fromPermalink} />
              ))}
            </div>
          </>
        )}
      </main>
    </BlogPageShell>
  );
}
