"use client";

import { useLocale, useTranslations } from "next-intl";
import Image from "next/image";
import Link from "next/link";

import { excerptClient } from "@/lib/excerpt-client";
import { formatDate, readingMinutes } from "@/lib/format";
import type { Post } from "@/lib/types";

/** Variante client della `FeedPostCard` (stesso layout, mockup 1c/3e) per
 * gli elenchi caricati nel browser: feed "Seguiti" e tab del profilo. */
export function FeedPostCardClient({ post, blogTitle, showBlog = true }: { post: Post; blogTitle?: string; showBlog?: boolean }) {
  const t = useTranslations("Feed");
  const locale = useLocale();
  return (
    <article className="grid gap-4 border-b border-border py-5 last:border-0 md:grid-cols-[minmax(0,1fr)_128px] md:gap-7 md:py-6">
      <div className="flex min-w-0 flex-col gap-2">
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
        <h3 className="m-0 font-serif text-xl font-medium leading-tight text-pretty md:text-[23px]">
          <Link href={post.permalink} className="text-foreground no-underline hover:text-primary">
            {post.title}
          </Link>
        </h3>
        <p className="m-0 text-[15px] leading-relaxed text-muted md:line-clamp-2">{excerptClient(post.content, 180)}</p>
        <div className="mt-1 flex flex-wrap items-center gap-3.5 text-[13px] text-muted">
          {post.category && <span className="font-medium">{post.category.name}</span>}
          <span>{t("minutes", { min: readingMinutes(post.content) })}</span>
          {post.notes.length > 0 && <span>{t("notes", { count: post.notes.length })}</span>}
          {post.tags.slice(0, 3).map((tag) => (
            <Link key={tag} href={`/?tag=${encodeURIComponent(tag)}`} className="text-muted no-underline hover:text-primary">
              #{tag}
            </Link>
          ))}
        </div>
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
