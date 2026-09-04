import type { Metadata } from "next";
import { notFound } from "next/navigation";

import { SiteHeader } from "@/components/SiteHeader";
import { excerpt, renderMarkdown } from "@/lib/markdown";
import { getPublicPlatformPage } from "@/lib/server-api";

interface PageParams {
  slug: string;
}

/** Pagina statica pubblica del sito principale (Chi siamo, Contatti,
 * Privacy, ...), gestita da Amministratore/Super Admin (CLAUDE.md #1).
 * Permalink dedicato `/pages/{slug}`, non alla radice, per non collidere con
 * gli slug dei blog raggiungibili senza sottodominio su `/{blog_slug}/...`. */
export async function generateMetadata({
  params,
  searchParams,
}: {
  params: Promise<PageParams>;
  searchParams: Promise<{ locale?: string }>;
}): Promise<Metadata> {
  const { slug } = await params;
  const { locale = "it" } = await searchParams;
  const page = await getPublicPlatformPage(slug, locale);
  if (!page) return {};
  return {
    title: page.title,
    description: excerpt(page.content),
  };
}

export default async function PublicPlatformPagePage({
  params,
  searchParams,
}: {
  params: Promise<PageParams>;
  searchParams: Promise<{ locale?: string }>;
}) {
  const { slug } = await params;
  const { locale = "it" } = await searchParams;
  const page = await getPublicPlatformPage(slug, locale);
  if (!page) notFound();

  const html = await renderMarkdown(page.content);

  return (
    <div className="flex flex-1 flex-col">
      <SiteHeader />
      <main className="mx-auto w-full max-w-2xl flex-1 px-6 py-12">
        <h1 className="mt-6 font-serif text-5xl font-semibold leading-tight text-foreground">{page.title}</h1>

        <div className="notturni-prose mt-10 text-lg leading-relaxed" dangerouslySetInnerHTML={{ __html: html }} />
      </main>
    </div>
  );
}
