import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import Link from "next/link";
import { notFound } from "next/navigation";

import { BlogPageShell } from "@/components/blog/BlogPageShell";
import { BlogHeaderActions } from "@/components/blog/PostHeaderActions";
import { FeedPostCard } from "@/components/FeedPostCard";
import { BlogHeader } from "@/components/shell/BlogHeader";
import { FilterChip } from "@/components/ui/Pill";
import { SITE_HOST } from "@/lib/site";
import { getPublicBlog, getPublicBlogCategories, getPublicBlogConfig, getPublicBlogPosts } from "@/lib/server-api";

interface PageParams {
  blogSlug: string;
}

export async function generateMetadata({ params }: { params: Promise<PageParams> }): Promise<Metadata> {
  const { blogSlug } = await params;
  const blog = await getPublicBlog(blogSlug);
  if (!blog) return {};
  const description = blog.description ?? blog.subtitle ?? undefined;
  return {
    title: blog.title,
    description,
    alternates: { canonical: `/${blogSlug}` },
    robots: blog.search_indexing_enabled ? undefined : { index: false, follow: false },
    openGraph: { title: blog.title, description, type: "website", url: `/${blogSlug}` },
  };
}

const hue = (s: string) => `oklch(0.55 0.06 ${[...s].reduce((a, c) => a + c.charCodeAt(0), 0) % 360})`;

/** Home pubblica del blog (mockup 3f): hero con iniziale/titolo/sottotitolo/
 * descrizione, chip delle categorie e feed dei post, palette custom via
 * BlogPageShell. Autori, pubblicazioni e newsletter arrivano con B1/B9. */
export default async function BlogHomePage({
  params,
  searchParams,
}: {
  params: Promise<PageParams>;
  searchParams: Promise<{ category?: string }>;
}) {
  const [{ blogSlug }, { category }] = await Promise.all([params, searchParams]);
  const [blog, posts, categories, config, t] = await Promise.all([
    getPublicBlog(blogSlug),
    getPublicBlogPosts(blogSlug),
    getPublicBlogCategories(blogSlug),
    getPublicBlogConfig(blogSlug),
    getTranslations("BlogPage"),
  ]);
  if (!blog || !posts) notFound();
  const visible = category ? posts.filter((p) => p.category?.slug === category) : posts;

  return (
    <BlogPageShell config={config}>
      <BlogHeader slug={blogSlug} name={blog.title} current="posts" actions={<BlogHeaderActions slug={blogSlug} />} />
      <main className="mx-auto w-full max-w-[1184px] flex-1 px-5 py-10 lg:px-12 lg:py-14">
        <header className="grid gap-6 md:grid-cols-[72px_minmax(0,1fr)] md:gap-7">
          <span
            className="grid h-[72px] w-[72px] place-items-center rounded-2xl font-serif text-3xl text-white"
            style={{ background: hue(blog.slug) }}
            aria-hidden="true"
          >
            {blog.title[0]}
          </span>
          <div className="flex flex-col gap-2">
            <h1 className="font-serif text-[34px] font-medium leading-[1.12] tracking-tight text-foreground md:text-[44px]">
              {blog.title}
            </h1>
            {blog.subtitle && <p className="text-lg text-muted md:text-xl">{blog.subtitle}</p>}
            {blog.description && <p className="max-w-[680px] text-[15px] leading-relaxed text-foreground/80">{blog.description}</p>}
            <span className="font-mono text-xs text-muted">
              {blog.slug}.{SITE_HOST}
            </span>
          </div>
        </header>

        <div className="mt-10 grid gap-10 lg:grid-cols-[minmax(0,1fr)_300px] lg:gap-16">
          <section className="min-w-0">
            <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border pb-3">
              <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("latest")}</span>
              {categories.length > 0 && (
                <div className="flex gap-1.5 overflow-x-auto">
                  <Link href={`/${blogSlug}`} className="no-underline">
                    <FilterChip active={!category}>{t("allPosts")}</FilterChip>
                  </Link>
                  {categories.map((c) => (
                    <Link key={c.id} href={`/${blogSlug}?category=${encodeURIComponent(c.slug)}`} className="no-underline">
                      <FilterChip active={category === c.slug}>{c.name}</FilterChip>
                    </Link>
                  ))}
                </div>
              )}
            </div>
            {visible.length === 0 ? (
              <p className="py-10 text-center text-sm text-muted">{t("noPosts")}</p>
            ) : (
              visible.map((post) => <FeedPostCard key={post.id} post={post} showBlog={false} />)
            )}
          </section>
          <aside className="flex flex-col gap-6 text-sm">
            <div className="flex flex-col gap-2 rounded-xl border border-border bg-surface px-[18px] py-4">
              <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("aboutBlog")}</span>
              <span className="text-[13px] leading-relaxed text-muted">
                {t("postsCount", { count: posts.length })} · {t("language", { lang: blog.default_locale.toUpperCase() })}
              </span>
              <Link href={`/${blogSlug}/bibliografia`} className="text-[13px] font-medium text-primary no-underline hover:underline">
                {t("bibliographyLink")}
              </Link>
            </div>
          </aside>
        </div>
      </main>
    </BlogPageShell>
  );
}
