import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";

import { SiteHeader } from "@/components/SiteHeader";
import { getBlogLinksBibliography, getPublicBlog } from "@/lib/server-api";

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
  return { title: blog ? `Link — ${blog.title}` : "Link" };
}

/** CLAUDE.md #4: come la bibliografia delle note (/[blogSlug]/bibliografia),
 * ma per i link citati nei post pubblicati. */
export default async function BlogLinksBibliographyPage({
  params,
}: {
  params: Promise<PageParams>;
}) {
  const { blogSlug } = await params;
  const [blog, entries] = await Promise.all([getPublicBlog(blogSlug), getBlogLinksBibliography(blogSlug)]);
  if (!blog || !entries) notFound();

  return (
    <div className="flex flex-1 flex-col">
      <SiteHeader />
      <main className="mx-auto w-full max-w-2xl flex-1 px-6 py-12">
        <Link href={`/${blogSlug}`} className="text-sm text-muted hover:text-foreground">
          ← {blogSlug}
        </Link>

        <h1 className="mt-6 font-serif text-4xl font-semibold leading-tight text-foreground">Link</h1>
        <p className="mt-2 text-sm text-muted">
          Tutti i link citati nei post di <span className="text-foreground">{blog.title}</span>, con
          l&apos;elenco dei post che li usano e la data di pubblicazione.
        </p>

        {entries.length === 0 ? (
          <p className="mt-10 text-sm text-muted">Nessun link, per ora.</p>
        ) : (
          <ol className="mt-10 space-y-6">
            {entries.map((entry, i) => (
              <li key={i} className="border-b border-border pb-6 last:border-0">
                <a
                  href={entry.url}
                  target="_blank"
                  rel="noopener noreferrer nofollow"
                  className="text-base text-primary hover:underline"
                >
                  {entry.link_text || entry.url}
                </a>
                <p className="mt-1 text-xs text-muted">{entry.url}</p>
                <p className="mt-2 text-xs text-muted">
                  Citato in:{" "}
                  {entry.citations.map((c, j) => (
                    <span key={`${c.permalink}-${j}`}>
                      {j > 0 && ", "}
                      <Link href={c.permalink} className="text-primary hover:underline">
                        {c.post_title}
                        {c.locale !== blog.default_locale ? ` (${c.locale})` : ""}
                      </Link>
                      {c.used_at && <span> ({new Date(c.used_at).toLocaleDateString("it-IT")})</span>}
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
