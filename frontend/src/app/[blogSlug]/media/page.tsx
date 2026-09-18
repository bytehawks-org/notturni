import type { Metadata } from "next";
import { getLocale, getTranslations } from "next-intl/server";
import Link from "next/link";
import { notFound } from "next/navigation";

import { BlogPageShell } from "@/components/blog/BlogPageShell";
import { BlogStateNotice, blogIsOffline } from "@/components/blog/BlogStateNotice";
import { BlogHeaderActions } from "@/components/blog/PostHeaderActions";
import { ExpandIcon } from "@/components/editor/icons";
import { BlogHeader } from "@/components/shell/BlogHeader";
import { FilterChip } from "@/components/ui/Pill";
import { SENSITIVITY_CATEGORY_LABELS } from "@/lib/content-media";
import { blogLinks } from "@/lib/blog-path";
import { formatDate } from "@/lib/format";
import { getBlogMediaBibliography, getPublicBlog, getPublicBlogConfig, getPublicPublications } from "@/lib/server-api";
import type { MediaBibliographyEntry } from "@/lib/types";

interface PageParams {
  blogSlug: string;
}

export async function generateMetadata({ params }: { params: Promise<PageParams> }): Promise<Metadata> {
  const { blogSlug } = await params;
  const [blog, t] = await Promise.all([getPublicBlog(blogSlug), getTranslations("BlogNav")]);
  if (!blog) return { title: t("media") };
  const title = `${t("media")} — ${blog.title}`;
  return {
    title,
    alternates: { canonical: `/${blogSlug}/media` },
    robots: blog.search_indexing_enabled ? undefined : { index: false, follow: false },
    openGraph: { title, type: "website", url: `/${blogSlug}/media` },
  };
}

function MediaFigure({
  entry,
  locale,
  defaultLocale,
  resolvePermalink,
  expandLabel,
}: {
  entry: MediaBibliographyEntry;
  locale: string;
  defaultLocale: string;
  resolvePermalink: (permalink: string) => string;
  expandLabel: string;
}) {
  // `is_sensitive`, non `categories.length > 0`: un'immagine segnalata
  // dalla sola automoderazione (o dal modal senza una categoria specifica
  // scelta) ha `categories` vuoto ma è comunque sensibile — vedi
  // backend/app/models/post_media.py e la pagina del post, che la sfoca
  // già in base allo stesso flag.
  const sensitive = entry.is_sensitive;
  const first = entry.citations[0];
  return (
    <figure className="flex min-w-0 flex-col gap-2">
      {sensitive ? (
        // <label> per l'associazione nativa label→checkbox (vedi pagina del post).
        <label className="sensitive-image-wrapper relative block aspect-square overflow-hidden rounded-[10px] border border-border bg-surface md:aspect-[4/3]">
          <input type="checkbox" className="sensitive-image-toggle" />
          {/* eslint-disable-next-line @next/next/no-img-element -- URL storage esterno */}
          <img src={entry.url} alt={entry.alt_text} className="h-full w-full object-cover" />
          <span className="sensitive-image-overlay">{SENSITIVITY_CATEGORY_LABELS[entry.categories[0] ?? "other"]}</span>
          <button type="button" className="lightbox-expand-btn" data-lightbox-src={entry.url} data-lightbox-alt={entry.alt_text} aria-label={expandLabel}>
            <ExpandIcon />
          </button>
        </label>
      ) : (
        <span className="relative block aspect-square overflow-hidden rounded-[10px] border border-border bg-surface md:aspect-[4/3]">
          {/* eslint-disable-next-line @next/next/no-img-element -- URL storage esterno */}
          <img
            src={entry.url}
            alt={entry.alt_text}
            data-lightbox="1"
            tabIndex={0}
            role="button"
            // `alt_text` può essere vuoto: senza un fallback il controllo da
            // tastiera resterebbe senza nome accessibile (bug segnalato
            // dalla review Copilot).
            aria-label={entry.alt_text || expandLabel}
            className="h-full w-full object-cover"
          />
        </span>
      )}
      <figcaption className="flex flex-col gap-0.5 text-[13px] leading-snug">
        <span className="truncate font-medium">{entry.alt_text}</span>
        {first && (
          <span className="truncate text-muted">
            <Link href={resolvePermalink(first.permalink)} className="no-underline hover:underline">
              {first.post_title}
              {first.locale !== defaultLocale ? ` (${first.locale})` : ""}
            </Link>
            {first.used_at && ` · ${formatDate(first.used_at, locale, { day: "numeric", month: "short", year: "numeric" })}`}
            {entry.citations.length > 1 && ` +${entry.citations.length - 1}`}
          </span>
        )}
      </figcaption>
    </figure>
  );
}

/** /{blog}/media (mockup 2b/3a): griglia delle immagini citate nei post
 * pubblicati; vista "per post" raggruppa per primo post che le usa. */
export default async function BlogMediaBibliographyPage({
  params,
  searchParams,
}: {
  params: Promise<PageParams>;
  searchParams: Promise<{ view?: string }>;
}) {
  const [{ blogSlug }, { view }] = await Promise.all([params, searchParams]);
  const [blog, entries, config, t, locale, links] = await Promise.all([
    getPublicBlog(blogSlug),
    getBlogMediaBibliography(blogSlug),
    getPublicBlogConfig(blogSlug),
    getTranslations("MediaPage"),
    getLocale(),
    blogLinks(blogSlug),
  ]);
  if (!blog) notFound();
  const hasPublications = (await getPublicPublications(blogSlug).catch(() => [])).length > 0;
  if (blogIsOffline(blog)) {
    return (
      <BlogPageShell config={config}>
        <BlogHeader basePath={links.basePath} name={blog.title} hasPublications={hasPublications} current="media" />
        <BlogStateNotice blog={blog} />
      </BlogPageShell>
    );
  }
  if (!entries) notFound();

  const byPost = new Map<string, { title: string; items: MediaBibliographyEntry[] }>();
  for (const entry of entries) {
    const first = entry.citations[0];
    const key = first?.permalink ?? "";
    const group = byPost.get(key) ?? { title: first?.post_title ?? "", items: [] };
    group.items.push(entry);
    byPost.set(key, group);
  }

  return (
    <BlogPageShell config={config}>
      <BlogHeader basePath={links.basePath} name={blog.title} hasPublications={hasPublications} current="media" actions={<BlogHeaderActions slug={blogSlug} />} />
      <main className="mx-auto w-full max-w-[1184px] flex-1 px-5 py-10 lg:px-12 lg:py-14">
        <div className="flex flex-col gap-1.5">
          <h1 className="font-serif text-[34px] font-medium leading-[1.12] tracking-tight md:text-[40px]">{t("title")}</h1>
          <p className="text-muted md:text-base">{t("subtitle", { count: entries.length })}</p>
        </div>
        <div className="mt-6 flex gap-1.5 border-b border-border pb-3">
          <Link href={`${links.basePath}/media`} className="no-underline">
            <FilterChip active={view !== "post"}>{t("grid")}</FilterChip>
          </Link>
          <Link href={`${links.basePath}/media?view=post`} className="no-underline">
            <FilterChip active={view === "post"}>{t("byPost")}</FilterChip>
          </Link>
        </div>
        {entries.length === 0 ? (
          <p className="py-10 text-center text-sm text-muted">{t("empty")}</p>
        ) : view === "post" ? (
          <div className="mt-6 flex flex-col gap-8">
            {[...byPost.entries()].map(([permalink, group]) => (
              <section key={permalink} className="flex flex-col gap-3">
                <Link href={links.fromPermalink(permalink)} className="font-serif text-[19px] text-foreground no-underline hover:text-primary">
                  {group.title}
                </Link>
                <div className="grid grid-cols-2 gap-3 md:gap-5 lg:grid-cols-4">
                  {group.items.map((entry, i) => (
                    <MediaFigure key={i} entry={entry} locale={locale} defaultLocale={blog.default_locale} resolvePermalink={links.fromPermalink} expandLabel={t("expandImage")} />
                  ))}
                </div>
              </section>
            ))}
          </div>
        ) : (
          <div className="mt-6 grid grid-cols-2 gap-3 md:gap-5 lg:grid-cols-4">
            {entries.map((entry, i) => (
              <MediaFigure key={i} entry={entry} locale={locale} defaultLocale={blog.default_locale} resolvePermalink={links.fromPermalink} expandLabel={t("expandImage")} />
            ))}
          </div>
        )}
      </main>
    </BlogPageShell>
  );
}
