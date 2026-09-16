import { getLocale, getTranslations } from "next-intl/server";
import Image from "next/image";
import Link from "next/link";

import { formatDate, readingMinutes } from "@/lib/format";
import { excerpt } from "@/lib/markdown";
import type { Post } from "@/lib/types";

/** Variante di disposizione (`typography.layout` del blog, mockup 2e/3f):
 * `standard` = riga con copertina piccola a lato (default), `magazine` =
 * copertina grande in testa alla card, `minimal` = lista compatta senza
 * copertina né estratto. Solo `/[blogSlug]` la passa esplicitamente — il
 * feed di piattaforma (aggregato su più blog) resta sempre `standard`. */
export type FeedPostCardVariant = "standard" | "magazine" | "minimal";

/** Riga del feed su / e /[blogSlug] (mockup 1c, 4a, 4b, 3e): autore, blog,
 * data, titolo, estratto, categoria, tempo di lettura, note, tag, copertina. */
export async function FeedPostCard({
  post,
  blogTitle,
  showBlog = true,
  variant = "standard",
}: {
  post: Post;
  blogTitle?: string;
  showBlog?: boolean;
  variant?: FeedPostCardVariant;
}) {
  const [t, locale] = await Promise.all([getTranslations("Feed"), getLocale()]);
  const meta = (
    <div className="flex flex-wrap items-center gap-2 text-[13px] text-muted">
      <span className="grid h-5 w-5 place-items-center rounded-full bg-primary/60 font-serif text-[11px] text-background">
        {post.author_display_name[0]?.toUpperCase()}
      </span>
      <span className="font-medium text-foreground">{post.author_display_name}</span>
      {showBlog && (
        <span>
          {t("in")}{" "}
          <Link href={`/${post.blog_slug}`} className="no-underline hover:underline">
            {blogTitle ?? post.blog_slug}
          </Link>
        </span>
      )}
      {post.published_at && (
        <>
          <span>·</span>
          <span>{formatDate(post.published_at, locale, { day: "numeric", month: "short", year: "numeric" })}</span>
        </>
      )}
    </div>
  );
  const title = (
    <h3 className="m-0 font-serif text-xl font-medium leading-tight text-pretty md:text-[23px]">
      <Link href={post.permalink} className="text-foreground no-underline hover:text-primary">
        {post.title}
      </Link>
    </h3>
  );
  const footer = (
    <div className="mt-1 flex flex-wrap items-center gap-3.5 text-[13px] text-muted">
      {post.category && (
        <Link href={`/?category=${encodeURIComponent(post.category.slug)}`} className="font-medium no-underline hover:underline">
          {post.category.name}
        </Link>
      )}
      <span>{t("minutes", { min: readingMinutes(post.content) })}</span>
      {post.notes.length > 0 && <span>{t("notes", { count: post.notes.length })}</span>}
      {post.tags.slice(0, 3).map((tag) => (
        <Link key={tag} href={`/?tag=${encodeURIComponent(tag)}`} className="text-muted no-underline hover:text-primary">
          #{tag}
        </Link>
      ))}
    </div>
  );

  if (variant === "minimal") {
    return (
      <article className="flex flex-col gap-1.5 border-b border-border py-3.5 last:border-0">
        {meta}
        {title}
        {footer}
      </article>
    );
  }

  if (variant === "magazine") {
    return (
      <article className="flex flex-col gap-3 border-b border-border py-5 last:border-0">
        {post.cover_image_url && (
          <Link href={post.permalink} className="relative block aspect-[16/9] w-full overflow-hidden rounded-lg border border-border">
            <Image
              src={post.cover_image_url}
              alt=""
              fill
              sizes="(min-width: 768px) 640px, 100vw"
              unoptimized
              className={`object-cover ${post.cover_image_is_sensitive ? "blur-md" : ""}`}
            />
          </Link>
        )}
        {meta}
        {title}
        <p className="m-0 text-[15px] leading-relaxed text-muted line-clamp-3">{excerpt(post.content, 220)}</p>
        {footer}
      </article>
    );
  }

  return (
    <article className="grid gap-4 border-b border-border py-5 last:border-0 md:grid-cols-[minmax(0,1fr)_128px] md:gap-7 md:py-6">
      <div className="flex min-w-0 flex-col gap-2">
        {meta}
        {title}
        <p className="m-0 text-[15px] leading-relaxed text-muted md:line-clamp-2">{excerpt(post.content, 180)}</p>
        {footer}
      </div>
      {post.cover_image_url && (
        <Link href={post.permalink} className="relative hidden h-24 w-full overflow-hidden rounded-lg border border-border md:block">
          <Image
            src={post.cover_image_url}
            alt=""
            fill
            sizes="128px"
            unoptimized
            className={`object-cover ${post.cover_image_is_sensitive ? "blur-md" : ""}`}
          />
        </Link>
      )}
    </article>
  );
}
