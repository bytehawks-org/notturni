import type { Metadata, ResolvingMetadata } from "next";
import { withSiteOpenGraph } from "@/lib/metadata";
import { getTranslations } from "next-intl/server";
import { notFound } from "next/navigation";

import { BlogPageShell } from "@/components/blog/BlogPageShell";
import { BlogHeader } from "@/components/shell/BlogHeader";
import { currentBlogLinks } from "@/lib/blog-links";
import { excerpt, renderMarkdown } from "@/lib/markdown";
import { getPrimaryBlogSlug, getPublicBlog, getPublicBlogConfig, getPublicPage, getPublicPublications } from "@/lib/server-api";
import { spaceHref } from "@/lib/site";

interface PageParams {
  blogSlug: string;
  pageSlug: string;
}

/** Pagina statica pubblica di un blog (CLAUDE.md #1, feature opt-in). Niente
 * data/tag/categoria a differenza del post pubblico
 * ([blogSlug]/[postSlug]/page.tsx) — le pagine statiche non sono
 * cronologiche. */
export async function generateMetadata({
  params,
  searchParams,
}: {
  params: Promise<PageParams>;
  searchParams: Promise<{ locale?: string }>;
}, parent: ResolvingMetadata): Promise<Metadata> {
  const { blogSlug, pageSlug } = await params;
  const { locale = "it" } = await searchParams;
  const [page, blog, primary] = await Promise.all([getPublicPage(blogSlug, pageSlug, locale), getPublicBlog(blogSlug), getPrimaryBlogSlug()]);
  if (!page) return {};
  const description = excerpt(page.content);
  const href = spaceHref(blogSlug, `/pagina/${pageSlug}`, primary);
  return {
    title: page.title,
    description,
    alternates: { canonical: href },
    robots: blog?.search_indexing_enabled === false ? { index: false, follow: false } : undefined,
    openGraph: await withSiteOpenGraph(parent, { title: page.title, description, type: "website", url: href }),
  };
}

export default async function PublicBlogPagePage({
  params,
  searchParams,
}: {
  params: Promise<PageParams>;
  searchParams: Promise<{ locale?: string }>;
}) {
  const { blogSlug, pageSlug } = await params;
  const { locale = "it" } = await searchParams;
  const [page, blog, links, config, publications] = await Promise.all([
    getPublicPage(blogSlug, pageSlug, locale),
    getPublicBlog(blogSlug),
    currentBlogLinks(blogSlug),
    getPublicBlogConfig(blogSlug),
    getPublicPublications(blogSlug).catch(() => []),
  ]);
  if (!page) notFound();

  const tPost = await getTranslations("Post");
  const html = await renderMarkdown(page.content, {
    mentions: page.mentions_enabled,
    expandImageLabel: tPost("expandImage"),
    copyCodeLabel: tPost("copyCode"),
    copiedCodeLabel: tPost("copiedCode"),
  });

  return (
    <BlogPageShell config={config}>
      <BlogHeader basePath={links.basePath} homeHref={links.homeHref} name={blog?.title ?? blogSlug} current="posts" hasPublications={publications.length > 0} />
      <main className="mx-auto w-full max-w-2xl flex-1 px-6 py-12">
        <h1 className="font-serif text-5xl font-semibold leading-tight text-foreground">{page.title}</h1>

        <div className="notturni-prose mt-10 text-lg leading-relaxed" dangerouslySetInnerHTML={{ __html: html }} />
      </main>
    </BlogPageShell>
  );
}
