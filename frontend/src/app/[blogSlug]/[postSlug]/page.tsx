import type { CSSProperties } from "react";

import type { Metadata } from "next";
import { getLocale, getTranslations } from "next-intl/server";
import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";

import { BlogPageShell, blogMeasure } from "@/components/blog/BlogPageShell";
import { JsonLd } from "@/components/blog/JsonLd";
import { PostActions } from "@/components/blog/PostActions";
import { BlogHeaderActions } from "@/components/blog/PostHeaderActions";
import { PostToc } from "@/components/blog/PostToc";
import { SensitiveImage } from "@/components/blog/SensitiveImage";
import { ChapterNav, ChapterProgress } from "@/components/publications/PublicationIndex";
import { ReadBeacon } from "@/components/blog/ReadBeacon";
import { ReportButton } from "@/components/blog/ReportDialog";
import { CommentsSection } from "@/components/CommentsSection";
import { FragmentReader } from "@/components/FragmentReader";
import { BlogHeader } from "@/components/shell/BlogHeader";
import { TagPills } from "@/components/TagPills";
import { formatDate, readingMinutes } from "@/lib/format";
import { languageName } from "@/lib/languages";
import { excerpt, renderPost } from "@/lib/markdown";
import { getPublicBlog, getPublicBlogConfig, getPublicPostByPermalink, getPublicPostTranslations, getPublicPublication, getPublicPublications } from "@/lib/server-api";
import { SITE_URL } from "@/lib/site";
import { blogLinks } from "@/lib/blog-path";

interface PageParams {
  blogSlug: string;
  postSlug: string;
}

export async function generateMetadata({ params }: { params: Promise<PageParams> }): Promise<Metadata> {
  const { blogSlug, postSlug } = await params;
  const post = await getPublicPostByPermalink(blogSlug, postSlug);
  if (!post) return {};
  const description = excerpt(post.content);
  return {
    title: post.title,
    description,
    alternates: { canonical: post.permalink },
    // Opt-out per crawler (app/domain/seo.py::effective_search_indexing) —
    // il blocco dei crawler IA/LLM specifici passa invece da robots.txt
    // (frontend/src/app/robots.ts), non da questo meta tag.
    robots: post.effective_search_indexing_enabled ? undefined : { index: false, follow: false },
    openGraph: {
      title: post.title,
      description,
      type: "article",
      url: post.permalink,
      publishedTime: post.published_at ?? undefined,
      authors: [post.author_display_name],
      images: post.cover_image_url && !post.cover_image_is_sensitive ? [post.cover_image_url] : undefined,
    },
  };
}

/** Pagina pubblica del post (mockup 1a desktop / 1b mobile): griglia
 * `1fr 680px 1fr` da xl con indice a sinistra e note/categoria/lingue a
 * destra; colonna singola sotto, con il menu frammenti come bottom-sheet. */
export default async function PublicPostPage({ params }: { params: Promise<PageParams> }) {
  const { blogSlug, postSlug } = await params;
  const [post, blog, config, t, locale, links] = await Promise.all([
    getPublicPostByPermalink(blogSlug, postSlug),
    getPublicBlog(blogSlug),
    getPublicBlogConfig(blogSlug),
    getTranslations("PostPage"),
    getLocale(),
    blogLinks(blogSlug),
  ]);
  const basePath = links.basePath;
  if (!post) notFound();
  const measure = blogMeasure(config);
  const articleWidth = measure === "narrow" ? "560px" : "680px";
  const tPost = await getTranslations("Post");
  const translations = (await getPublicPostTranslations(post.id).catch(() => [])).filter(
    (tr) => tr.status === "published"
  );
  // B9: capitolo di una pubblicazione → barra di avanzamento e prev/next
  const [publication, publications] = await Promise.all([
    post.publication ? getPublicPublication(blogSlug, post.publication.name).catch(() => null) : Promise.resolve(null),
    getPublicPublications(blogSlug).catch(() => []),
  ]);
  const chapterIndex = publication ? publication.chapters.findIndex((c) => c.post_id === post.id) : -1;
  const chapter = chapterIndex >= 0 && publication ? { current: chapterIndex + 1, total: publication.chapters.length, prev: publication.chapters[chapterIndex - 1], next: publication.chapters[chapterIndex + 1] } : null;

  const { html, headings } = await renderPost(post.content, {
    mentions: post.mentions_enabled,
    notes: post.notes,
    footnoteLabels: { title: tPost("notes"), backToText: tPost("backToText") },
    expandImageLabel: tPost("expandImage"),
    copyCodeLabel: tPost("copyCode"),
    copiedCodeLabel: tPost("copiedCode"),
  });
  const blogTitle = blog?.title ?? blogSlug;
  const minutes = readingMinutes(post.content);
  const publishedDate = post.published_at ? formatDate(post.published_at, locale) : null;
  const citation = `${post.author_display_name}, “${post.title}”, ${blogTitle}${publishedDate ? `, ${publishedDate}.` : "."}`;

  // Dati strutturati Schema.org (SEO, todo/UX_REDESIGN.md): `dateModified`
  // non è tracciato separatamente da `created_at`/`published_at` nel modello
  // (nessun campo "ultima modifica" sul post), quindi resta assente invece
  // di riportare un valore non affidabile.
  const jsonLd = {
    "@context": "https://schema.org",
    "@type": "BlogPosting",
    headline: post.title,
    description: excerpt(post.content),
    url: `${SITE_URL}${post.permalink}`,
    mainEntityOfPage: `${SITE_URL}${post.permalink}`,
    datePublished: post.published_at ?? undefined,
    inLanguage: post.locale,
    author: { "@type": "Person", name: post.author_display_name },
    isPartOf: { "@type": "Blog", name: blogTitle, url: `${SITE_URL}/${blogSlug}` },
    ...(post.cover_image_url && !post.cover_image_is_sensitive ? { image: post.cover_image_url } : {}),
  };

  const rail = (
    <div className="flex flex-col gap-6 text-[13px]">
      {post.notes.length > 0 && (
        <div className="flex flex-col gap-2">
          <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">
            {t("footnotes", { count: post.notes.length })}
          </span>
          <ol className="flex list-decimal flex-col gap-1 pl-4 text-muted">
            {post.notes.slice(0, 5).map((n) => (
              <li key={n.idx}>
                <a href={`#fn-${n.idx}`} className="line-clamp-2 no-underline hover:text-foreground">
                  {n.content}
                </a>
              </li>
            ))}
          </ol>
          <Link href={`${basePath}/bibliografia`} className="text-primary no-underline hover:underline">
            {tPost("blogBibliography")}
          </Link>
        </div>
      )}
      {(post.category || translations.length > 1) && (
        <div className="flex flex-col gap-2">
          <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("about")}</span>
          <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-muted">
            {post.category && (
              <Link href={`/?category=${encodeURIComponent(post.category.slug)}`} className="font-medium text-foreground no-underline hover:text-primary">
                {post.category.name}
              </Link>
            )}
            {post.category && translations.length > 1 && <span>·</span>}
            {translations.map((tr, i) => (
              <span key={tr.id}>
                {i > 0 && <span className="mr-2">·</span>}
                {tr.locale === post.locale ? (
                  <span className="font-medium text-foreground">{languageName(tr.locale, locale)}</span>
                ) : (
                  <Link href={`${basePath}/${tr.slug}`} className="no-underline hover:text-foreground">
                    {languageName(tr.locale, locale)}
                  </Link>
                )}
              </span>
            ))}
          </div>
        </div>
      )}
    </div>
  );

  return (
    <BlogPageShell config={config}>
      <JsonLd data={jsonLd} />
      <BlogHeader basePath={basePath} name={blogTitle} current={chapter ? "publications" : "posts"} hasPublications={publications.length > 0} actions={<BlogHeaderActions slug={blogSlug} />} />
      {chapter && <ChapterProgress current={chapter.current} total={chapter.total} />}
      <main className="mx-auto w-full max-w-[1184px] flex-1 px-5 py-10 lg:px-12 lg:py-14">
        <div
          className="grid gap-10 xl:grid-cols-[minmax(0,1fr)_var(--blog-article-width)_minmax(0,1fr)] xl:gap-12"
          style={{ "--blog-article-width": articleWidth } as CSSProperties}
        >
          <aside className="hidden xl:block">
            <div className="sticky top-8">
              <PostToc headings={headings} />
            </div>
          </aside>

          <article className="mx-auto w-full min-w-0" style={{ maxWidth: articleWidth }}>
            <header className="flex flex-col gap-4">
              {chapter && publication && (
                <Link href={`${basePath}/pub/${publication.name}`} className="font-mono text-[11px] uppercase tracking-[.08em] text-primary no-underline hover:underline">
                  {publication.title} · {chapter.current}/{chapter.total}
                </Link>
              )}
              <h1 className="font-serif text-[34px] font-medium leading-[1.12] tracking-tight text-foreground md:text-[44px]">
                {post.title}
              </h1>
              <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-muted">
                <span className="flex items-center gap-2">
                  {post.author_avatar_url ? (
                    <Image
                      src={post.author_avatar_url}
                      alt=""
                      width={28}
                      height={28}
                      className="h-7 w-7 rounded-full object-cover"
                      unoptimized
                    />
                  ) : (
                    <span className="grid h-7 w-7 place-items-center rounded-full bg-primary font-serif text-sm text-background">
                      {post.author_display_name[0]?.toUpperCase()}
                    </span>
                  )}
                  <span className="font-medium text-foreground">{post.author_display_name}</span>
                </span>
                {publishedDate && <span>{publishedDate}</span>}
                <span>{t("minRead", { min: minutes })}</span>
                {post.category && (
                  <Link href={`/?category=${encodeURIComponent(post.category.slug)}`} className="no-underline hover:text-foreground">
                    {post.category.name}
                  </Link>
                )}
              </div>
              <div className="flex flex-wrap items-center gap-4">
                <PostActions permalink={links.fromPermalink(post.permalink)} title={post.title} citation={citation} />
                <ReportButton target={{ type: "post", id: post.id }} />
              </div>
            </header>

            {post.cover_image_url && (
              <SensitiveImage
                src={post.cover_image_url}
                sensitive={post.cover_image_is_sensitive}
                className="mt-8 block aspect-[16/9] w-full overflow-hidden rounded-xl object-cover"
                revealLabel={t("revealCover")}
                sensitiveLabel={t("sensitiveCover")}
                expandLabel={t("expandImage")}
              />
            )}

            <div className="mt-6 xl:hidden">
              <details className="rounded-xl border border-border bg-surface px-4 py-3 text-sm">
                <summary className="cursor-pointer font-medium text-foreground">{t("inThisPost")}</summary>
                <div className="mt-3">
                  <PostToc headings={headings} />
                </div>
              </details>
            </div>

            <div style={{ fontSize: "var(--blog-body-size)" }}>
              <FragmentReader
                postId={post.id}
                html={html}
                permalink={links.fromPermalink(post.permalink)}
                quoteAttribution={`${post.author_display_name}, ${post.title}`}
                className="notturni-prose notturni-prose--reading mt-8 leading-[1.7]"
              />
            </div>

            {post.tags.length > 0 && (
              <div className="mt-10">
                <TagPills tags={post.tags} />
              </div>
            )}

            <div className="mt-10 xl:hidden">{rail}</div>

            {chapter && publication && (
              <ChapterNav prev={chapter.prev} next={chapter.next} index={{ title: publication.title, href: `${basePath}/pub/${publication.name}` }} resolvePermalink={links.fromPermalink} />
            )}

            <CommentsSection postId={post.id} mode={post.effective_comments_mode} />
            <ReadBeacon postId={post.id} />

            <p className="mt-10 text-[13px] leading-relaxed text-muted">{t("privacyNote")}</p>
          </article>

          <aside className="hidden xl:block">
            <div className="sticky top-8">{rail}</div>
          </aside>
        </div>
      </main>
    </BlogPageShell>
  );
}
