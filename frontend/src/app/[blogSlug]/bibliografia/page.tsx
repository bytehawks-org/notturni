import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";

import { SiteHeader } from "@/components/SiteHeader";
import { renderNoteInline } from "@/lib/markdown";
import { getBlogBibliography, getPublicBlog } from "@/lib/server-api";

interface PageParams {
  blogSlug: string;
}

export async function generateMetadata({
  params,
}: {
  params: Promise<PageParams>;
}): Promise<Metadata> {
  const { blogSlug } = await params;
  const blog = await getPublicBlog(blogSlug);
  return { title: blog ? `Bibliografia — ${blog.title}` : "Bibliografia" };
}

export default async function BlogBibliographyPage({
  params,
}: {
  params: Promise<PageParams>;
}) {
  const { blogSlug } = await params;
  const [blog, entries] = await Promise.all([
    getPublicBlog(blogSlug),
    getBlogBibliography(blogSlug),
  ]);
  if (!blog || !entries) notFound();

  return (
    <div className="flex flex-1 flex-col">
      <SiteHeader />
      <main className="mx-auto w-full max-w-2xl flex-1 px-6 py-12">
        <Link href={`/${blogSlug}`} className="text-sm text-muted hover:text-foreground">
          ← {blogSlug}
        </Link>

        <h1 className="mt-6 font-serif text-4xl font-semibold leading-tight text-foreground">
          Bibliografia
        </h1>
        <p className="mt-2 text-sm text-muted">
          Tutte le note a piè di pagina dei post di <span className="text-foreground">{blog.title}</span>,
          con l&apos;elenco dei post che le citano.
        </p>

        {entries.length === 0 ? (
          <p className="mt-10 text-sm text-muted">Nessuna nota, per ora.</p>
        ) : (
          <ol className="mt-10 space-y-6">
            {entries.map((entry, i) => (
              <li key={i} className="border-b border-border pb-6 last:border-0">
                <div
                  className="notturni-prose text-base leading-relaxed"
                  dangerouslySetInnerHTML={{ __html: renderNoteInline(entry.content) }}
                />
                <p className="mt-2 text-xs text-muted">
                  Citata in:{" "}
                  {entry.citations.map((c, j) => (
                    <span key={`${c.permalink}-${c.idx}`}>
                      {j > 0 && ", "}
                      <Link href={`${c.permalink}#fn-${c.idx}`} className="text-primary hover:underline">
                        {c.post_title}
                        {c.locale !== blog.default_locale ? ` (${c.locale})` : ""}
                      </Link>
                    </span>
                  ))}
                </p>
              </li>
            ))}
          </ol>
        )}
      </main>
    </div>
  );
}
