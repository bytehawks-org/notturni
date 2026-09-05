import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";

import { SiteHeader } from "@/components/SiteHeader";
import { SENSITIVITY_CATEGORY_LABELS } from "@/lib/content-media";
import { getBlogMediaBibliography, getPublicBlog } from "@/lib/server-api";

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
  return { title: blog ? `Media — ${blog.title}` : "Media" };
}

/** CLAUDE.md #4: come la bibliografia delle note (/[blogSlug]/bibliografia),
 * ma per i media citati nei post pubblicati — con la stessa immagine
 * sfocata/cliccabile delle pagine dei post per quelle segnalate (stesso
 * trucco CSS di frontend/src/lib/markdown.ts::wrapSensitiveImages, qui
 * scritto direttamente in JSX perché non passa dal rendering Markdown). */
export default async function BlogMediaBibliographyPage({
  params,
}: {
  params: Promise<PageParams>;
}) {
  const { blogSlug } = await params;
  const [blog, entries] = await Promise.all([getPublicBlog(blogSlug), getBlogMediaBibliography(blogSlug)]);
  if (!blog || !entries) notFound();

  return (
    <div className="flex flex-1 flex-col">
      <SiteHeader />
      <main className="mx-auto w-full max-w-2xl flex-1 px-6 py-12">
        <Link href={`/${blogSlug}`} className="text-sm text-muted hover:text-foreground">
          ← {blogSlug}
        </Link>

        <h1 className="mt-6 font-serif text-4xl font-semibold leading-tight text-foreground">Media</h1>
        <p className="mt-2 text-sm text-muted">
          Tutte le immagini usate nei post di <span className="text-foreground">{blog.title}</span>, con
          l&apos;elenco dei post che le citano e la data di pubblicazione.
        </p>

        {entries.length === 0 ? (
          <p className="mt-10 text-sm text-muted">Nessuna immagine, per ora.</p>
        ) : (
          <ol className="mt-10 space-y-8">
            {entries.map((entry, i) => (
              <li key={i} className="border-b border-border pb-8 last:border-0">
                {entry.categories.length > 0 ? (
                  <label className="sensitive-image-wrapper block max-w-sm">
                    <input type="checkbox" className="sensitive-image-toggle" />
                    {/* eslint-disable-next-line @next/next/no-img-element -- URL storage esterno */}
                    <img src={entry.url} alt={entry.alt_text} className="rounded-lg" />
                    <span className="sensitive-image-overlay">Contenuto sensibile — clicca per vedere</span>
                  </label>
                ) : (
                  // eslint-disable-next-line @next/next/no-img-element -- URL storage esterno
                  <img src={entry.url} alt={entry.alt_text} className="max-w-sm rounded-lg" />
                )}
                {entry.alt_text && <p className="mt-2 text-sm text-foreground/80">{entry.alt_text}</p>}
                {entry.categories.length > 0 && (
                  <p className="mt-1 text-xs text-muted">
                    Avviso: {entry.categories.map((c) => SENSITIVITY_CATEGORY_LABELS[c]).join(", ")}
                  </p>
                )}
                <p className="mt-2 text-xs text-muted">
                  Usata in:{" "}
                  {entry.citations.map((c, j) => (
                    <span key={`${c.permalink}-${j}`}>
                      {j > 0 && ", "}
                      <Link href={c.permalink} className="text-primary hover:underline">
                        {c.post_title}
                        {c.locale !== blog.default_locale ? ` (${c.locale})` : ""}
                      </Link>
                      {c.used_at && (
                        <span> ({new Date(c.used_at).toLocaleDateString("it-IT")})</span>
                      )}
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
