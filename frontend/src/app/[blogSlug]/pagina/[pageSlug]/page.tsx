import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";

import { SiteHeader } from "@/components/SiteHeader";
import { excerpt, renderMarkdown } from "@/lib/markdown";
import { getPublicPage } from "@/lib/server-api";

interface PageParams {
  blogSlug: string;
  pageSlug: string;
}

/** Pagina statica pubblica di un blog (CLAUDE.md #1, feature opt-in). Niente
 * data/tag/categoria a differenza del post pubblico
 * ([blogSlug]/[date]/[postSlug]/page.tsx) — le pagine statiche non sono
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
  const page = await getPublicPage(blogSlug, pageSlug, locale);
  if (!page) return {};
  return {
    title: page.title,
    description: excerpt(page.content),
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
  const page = await getPublicPage(blogSlug, pageSlug, locale);
  if (!page) notFound();

  const html = await renderMarkdown(page.content, { mentions: page.mentions_enabled });

  return (
    <div className="flex flex-1 flex-col">
      <SiteHeader />
      <main className="mx-auto w-full max-w-2xl flex-1 px-6 py-12">
        <Link href={`/${blogSlug}`} className="text-sm text-muted hover:text-foreground">
          ← {blogSlug}
        </Link>

        <h1 className="mt-6 font-serif text-5xl font-semibold leading-tight text-foreground">{page.title}</h1>

        <div className="notturni-prose mt-10 text-lg leading-relaxed" dangerouslySetInnerHTML={{ __html: html }} />
      </main>
    </div>
  );
}
