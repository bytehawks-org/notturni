import { useTranslations } from "next-intl";
import Link from "next/link";
import { excerpt } from "@/lib/markdown";
import type { Post } from "@/lib/types";

const fmt = (iso: string) => new Date(iso).toLocaleDateString("en-GB", { day: "numeric", month: "short", year: "numeric" });

/** Feed row used on / and /[blogSlug] (mockups 1c, 4a, 4b). Replaces the current FeedPostCard. */
export function FeedPostCard({ post }: { post: Post }) {
  const t = useTranslations("Feed");
  return (
    <article className="grid gap-4 border-b border-border py-5 md:grid-cols-[minmax(0,1fr)_128px] md:gap-7 md:py-6">
      <div className="flex min-w-0 flex-col gap-2">
        <div className="flex items-center gap-2 text-[13px] text-muted">
          {post.author_avatar_url
            ? /* eslint-disable-next-line @next/next/no-img-element */ <img src={post.author_avatar_url} alt="" className="h-5 w-5 rounded-full object-cover" />
            : <span className="h-5 w-5 rounded-full bg-primary/60" />}
          <span className="font-medium text-foreground">{post.author_display_name}</span>
          <span>{t("in")} <Link href={`/${post.blog_slug}`}>{post.blog_title ?? post.blog_slug}</Link></span>
          {post.published_at && <><span>·</span><span>{fmt(post.published_at)}</span></>}
        </div>
        <h3 className="m-0 font-serif text-xl font-medium leading-tight text-pretty md:text-[23px]"><Link href={post.permalink} className="text-foreground no-underline hover:text-primary">{post.title}</Link></h3>
        <p className="m-0 text-[15px] leading-relaxed text-muted md:line-clamp-2">{excerpt(post.content, 180)}</p>
        <div className="mt-1 flex flex-wrap items-center gap-3.5 text-[13px] text-muted">
          {post.category && <Link href={`/?category=${encodeURIComponent(post.category.slug)}`} className="font-medium">{post.category.name}</Link>}
          {post.reading_minutes != null && <span>{t("minutes", { min: post.reading_minutes })}</span>}
          {post.footnote_count ? <span>{t("notes", { count: post.footnote_count })}</span> : null}
          {post.tags.slice(0, 3).map((tag) => <Link key={tag} href={`/?tag=${encodeURIComponent(tag)}`} className="text-muted no-underline hover:text-primary">#{tag}</Link>)}
        </div>
      </div>
      {post.cover_image_url && (
        <Link href={post.permalink} className="hidden md:block">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img src={post.cover_image_url} alt="" className={`h-24 w-full rounded-lg border border-border object-cover ${post.cover_image_is_sensitive ? "blur-md" : ""}`} />
        </Link>
      )}
    </article>
  );
}
