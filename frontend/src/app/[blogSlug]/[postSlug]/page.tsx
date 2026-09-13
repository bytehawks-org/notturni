import type { Metadata } from "next";
import { getLocale, getTranslations } from "next-intl/server";
import Link from "next/link";
import { notFound } from "next/navigation";

import { PostActions } from "@/components/blog/PostActions";
import { BlogHeaderActions } from "@/components/blog/PostHeaderActions";
import { PostToc } from "@/components/blog/PostToc";
import { CommentsSection } from "@/components/CommentsSection";
import { FragmentReader } from "@/components/FragmentReader";
import { BlogHeader } from "@/components/shell/BlogHeader";
import { TagPills } from "@/components/TagPills";
import { formatDate, readingMinutes } from "@/lib/format";
import { languageName } from "@/lib/languages";
import { excerpt, renderPost } from "@/lib/markdown";
import { getPublicBlog, getPublicPostByPermalink, getPublicPostTranslations } from "@/lib/server-api";

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
  const [post, blog, t, locale] = await Promise.all([
    getPublicPostByPermalink(blogSlug, postSlug),
    getPublicBlog(blogSlug),
    getTranslations("PostPage"),
    getLocale(),
  ]);
  if (!post) notFound();
  const tPost = await getTranslations("Post");
  const translations = (await getPublicPostTranslations(post.id).catch(() => [])).filter(
    (tr) => tr.status === "published"
  );

  const { html, headings } = await renderPost(post.content, {
    mentions: post.mentions_enabled,
    notes: post.notes,
    footnoteLabels: { title: tPost("notes"), backToText: tPost("backToText") },
  });
  const blogTitle = blog?.title ?? blogSlug;
  const minutes = readingMinutes(post.content);
  const publishedDate = post.published_at ? formatDate(post.published_at, locale) : null;
  const citation = `${post.author_display_name}, “${post.title}”, ${blogTitle}${publishedDate ? `, ${publishedDate}.` : "."}`;

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
          <Link href={`/${blogSlug}/bibliografia`} className="text-primary no-underline hover:underline">
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
                  <Link href={`/${blogSlug}/${tr.slug}`} className="no-underline hover:text-foreground">
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
    <div className="flex flex-1 flex-col">
      <BlogHeader slug={blogSlug} name={blogTitle} current="posts" actions={<BlogHeaderActions slug={blogSlug} />} />
      <main className="mx-auto w-full max-w-[1184px] flex-1 px-5 py-10 lg:px-12 lg:py-14">
        <div className="grid gap-10 xl:grid-cols-[minmax(0,1fr)_680px_minmax(0,1fr)] xl:gap-12">
          <aside className="hidden xl:block">
            <div className="sticky top-8">
              <PostToc headings={headings} />
            </div>
          </aside>

          <article className="mx-auto w-full max-w-[680px] min-w-0">
            <header className="flex flex-col gap-4">
              <h1 className="font-serif text-[34px] font-medium leading-[1.12] tracking-tight text-foreground md:text-[44px]">
                {post.title}
              </h1>
              <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-muted">
                <span className="flex items-center gap-2">
                  <span className="grid h-7 w-7 place-items-center rounded-full bg-primary font-serif text-sm text-background">
                    {post.author_display_name[0]?.toUpperCase()}
                  </span>
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
              <PostActions permalink={post.permalink} title={post.title} citation={citation} />
            </header>

            {post.cover_image_url && post.cover_image_is_sensitive && (
              // Deve essere un <label> (non un <div>): il trucco CSS che toglie
              // la sfocatura al click si basa sull'associazione nativa
              // label→checkbox, che un <div> non offre (il checkbox non
              // riceverebbe mai il click, avendo pointer-events:none).
              <label className="sensitive-image-wrapper mt-8 aspect-[16/9] w-full overflow-hidden rounded-xl">
                <input type="checkbox" className="sensitive-image-toggle" aria-label={t("revealCover")} />
                {/* eslint-disable-next-line @next/next/no-img-element -- URL storage esterno */}
                <img src={post.cover_image_url} alt="" className="h-full w-full object-cover" />
                <span className="sensitive-image-overlay">{t("sensitiveCover")}</span>
              </label>
            )}
            {post.cover_image_url && !post.cover_image_is_sensitive && (
              // eslint-disable-next-line @next/next/no-img-element -- URL storage esterno
              <img src={post.cover_image_url} alt="" className="mt-8 aspect-[16/9] w-full rounded-xl object-cover" />
            )}

            <div className="mt-6 xl:hidden">
              <details className="rounded-xl border border-border bg-surface px-4 py-3 text-sm">
                <summary className="cursor-pointer font-medium text-foreground">{t("inThisPost")}</summary>
                <div className="mt-3">
                  <PostToc headings={headings} />
                </div>
              </details>
            </div>

            <FragmentReader
              postId={post.id}
              html={html}
              permalink={post.permalink}
              quoteAttribution={`${post.author_display_name}, ${post.title}`}
              className="notturni-prose notturni-prose--reading mt-8 text-lg leading-[1.7]"
            />

            {post.tags.length > 0 && (
              <div className="mt-10">
                <TagPills tags={post.tags} />
              </div>
            )}

            <div className="mt-10 xl:hidden">{rail}</div>

            <CommentsSection postId={post.id} mode={post.effective_comments_mode} />

            <p className="mt-10 text-[13px] leading-relaxed text-muted">{t("privacyNote")}</p>
          </article>

          <aside className="hidden xl:block">
            <div className="sticky top-8">{rail}</div>
          </aside>
        </div>
      </main>
    </div>
  );
}
