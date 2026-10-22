import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import { notFound } from "next/navigation";

import { BlogPageShell } from "@/components/blog/BlogPageShell";
import { BlogHeader } from "@/components/shell/BlogHeader";
import { blogBasePath } from "@/lib/blog-path";
import { excerpt, renderMarkdown } from "@/lib/markdown";
import { getPublicBlog, getPublicBlogConfig, getPublicPage, getPublicPublications } from "@/lib/server-api";

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
}): Promise<Metadata> {
  const { blogSlug, pageSlug } = await params;
  const { locale = "it" } = await searchParams;
  const [page, blog] = await Promise.all([getPublicPage(blogSlug, pageSlug, locale), getPublicBlog(blogSlug)]);
  if (!page) return {};
  const description = excerpt(page.content);
  return {
    title: page.title,
    description,
    alternates: { canonical: `/${blogSlug}/pagina/${pageSlug}` },
    robots: blog?.search_indexing_enabled === false ? { index: false, follow: false } : undefined,
    openGraph: { title: page.title, description, type: "website", url: `/${blogSlug}/pagina/${pageSlug}` },
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
  const [page, blog, basePath, config, publications] = await Promise.all([
    getPublicPage(blogSlug, pageSlug, locale),
    getPublicBlog(blogSlug),
    blogBasePath(blogSlug),
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
      <BlogHeader basePath={basePath} name={blog?.title ?? blogSlug} current="posts" hasPublications={publications.length > 0} />
      <main className="mx-auto w-full max-w-2xl flex-1 px-6 py-12">
        <h1 className="font-serif text-5xl font-semibold leading-tight text-foreground">{page.title}</h1>

        <div className="notturni-prose mt-10 text-lg leading-relaxed" dangerouslySetInnerHTML={{ __html: html }} />
      </main>
    </BlogPageShell>
  );
}
