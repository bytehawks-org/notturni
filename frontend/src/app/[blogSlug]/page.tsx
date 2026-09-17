import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";

import { BlogPageShell, blogLayout } from "@/components/blog/BlogPageShell";
import { BlogStateNotice, blogIsOffline } from "@/components/blog/BlogStateNotice";
import { BlogHeaderActions } from "@/components/blog/PostHeaderActions";
import { FeedPostCard } from "@/components/FeedPostCard";
import { SensitiveImage } from "@/components/blog/SensitiveImage";
import { BlogHeader } from "@/components/shell/BlogHeader";
import { FilterChip } from "@/components/ui/Pill";
import { SITE_HOST } from "@/lib/site";
import { blogLinks } from "@/lib/blog-path";
import { getPublicPublications, getPublicBlog, getPublicBlogCategories, getPublicBlogConfig, getPublicBlogPosts } from "@/lib/server-api";

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
    alternates: {
      canonical: `/${blogSlug}`,
      types: {
        "application/rss+xml": `/${blogSlug}/feed.xml`,
        "application/atom+xml": `/${blogSlug}/atom.xml`,
      },
    },
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
  const [blog, posts, categories, config, t, links] = await Promise.all([
    getPublicBlog(blogSlug),
    getPublicBlogPosts(blogSlug),
    getPublicBlogCategories(blogSlug),
    getPublicBlogConfig(blogSlug),
    getTranslations("BlogPage"),
    blogLinks(blogSlug),
  ]);
  const basePath = links.basePath;
  if (!blog) notFound();
  const hasPublications = (await getPublicPublications(blogSlug).catch(() => [])).length > 0;
  if (blogIsOffline(blog)) {
    return (
      <BlogPageShell config={config}>
        <BlogHeader basePath={basePath} name={blog.title} hasPublications={hasPublications} current="posts" />
        <BlogStateNotice blog={blog} />
      </BlogPageShell>
    );
  }
  if (!posts) notFound();
  const visible = category ? posts.filter((p) => p.category?.slug === category) : posts;
  const layout = blogLayout(config);

  return (
    <BlogPageShell config={config}>
      <BlogHeader basePath={basePath} name={blog.title} hasPublications={hasPublications} current="posts" actions={<BlogHeaderActions slug={blogSlug} />} />
      <main className="mx-auto w-full max-w-[1184px] flex-1 px-5 py-10 lg:px-12 lg:py-14">
        {blog.cover_image_url && (
          <SensitiveImage
            src={blog.cover_image_url}
            sensitive={blog.cover_image_is_sensitive}
            className="mb-8 block aspect-[21/6] w-full overflow-hidden rounded-xl object-cover"
            revealLabel={t("revealCover")}
            sensitiveLabel={t("sensitiveCover")}
            expandLabel={t("expandImage")}
          />
        )}
        <header className="grid gap-6 md:grid-cols-[72px_minmax(0,1fr)] md:gap-7">
          {blog.favicon_url ? (
            <Image
              src={blog.favicon_url}
              alt={blog.title}
              width={72}
              height={72}
              className="h-[72px] w-[72px] rounded-2xl object-cover"
              unoptimized
            />
          ) : (
            <span
              className="grid h-[72px] w-[72px] place-items-center rounded-2xl font-serif text-3xl text-white"
              style={{ background: hue(blog.slug) }}
              aria-hidden="true"
            >
              {blog.title[0]}
            </span>
          )}
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
                  <Link href={basePath || "/"} className="no-underline">
                    <FilterChip active={!category}>{t("allPosts")}</FilterChip>
                  </Link>
                  {categories.map((c) => (
                    <Link key={c.id} href={`${basePath || "/"}?category=${encodeURIComponent(c.slug)}`} className="no-underline">
                      <FilterChip active={category === c.slug}>{c.name}</FilterChip>
                    </Link>
                  ))}
                </div>
              )}
            </div>
            {visible.length === 0 ? (
              <p className="py-10 text-center text-sm text-muted">{t("noPosts")}</p>
            ) : layout === "magazine" ? (
              <div className="grid gap-6 sm:grid-cols-2">
                {visible.map((post) => (
                  <FeedPostCard key={post.id} post={post} showBlog={false} variant="magazine" resolvePermalink={links.fromPermalink} />
                ))}
              </div>
            ) : (
              visible.map((post) => (
                <FeedPostCard key={post.id} post={post} showBlog={false} variant={layout} resolvePermalink={links.fromPermalink} />
              ))
            )}
          </section>
          <aside className="flex flex-col gap-6 text-sm">
            {hasPublications && (
              <div className="flex flex-col gap-2 rounded-xl border border-border bg-surface px-[18px] py-4">
                <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("publications")}</span>
                <Link href={`${basePath}/pub`} className="text-[13px] font-medium text-primary no-underline hover:underline">
                  {t("publicationsLink")}
                </Link>
              </div>
            )}
            <div className="flex flex-col gap-2 rounded-xl border border-border bg-surface px-[18px] py-4">
              <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("aboutBlog")}</span>
              <span className="text-[13px] leading-relaxed text-muted">
                {t("postsCount", { count: posts.length })} · {t("language", { lang: blog.default_locale.toUpperCase() })}
              </span>
              <Link href={`${basePath}/bibliografia`} className="text-[13px] font-medium text-primary no-underline hover:underline">
                {t("bibliographyLink")}
              </Link>
              <Link href={`${basePath}/feed.xml`} className="text-[13px] font-medium text-primary no-underline hover:underline">
                {t("rssLink")}
              </Link>
            </div>
          </aside>
        </div>
      </main>
    </BlogPageShell>
  );
}
