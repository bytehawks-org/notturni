import Link from "next/link";

import { TagPills } from "@/components/TagPills";
import { excerpt } from "@/lib/markdown";
import type { Post } from "@/lib/types";

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString("it-IT", { day: "numeric", month: "long", year: "numeric" });
}

export function FeedPostCard({ post }: { post: Post }) {
  return (
    <article className="flex gap-5 border-b border-border/60 py-6 first:pt-0 last:border-0">
      {post.cover_image_url && (
        <Link href={post.permalink} className="shrink-0">
          {/* eslint-disable-next-line @next/next/no-img-element -- URL S3/MinIO esterno */}
          <img
            src={post.cover_image_url}
            alt=""
            className="h-24 w-24 rounded-lg object-cover sm:h-28 sm:w-28"
          />
        </Link>
      )}
      <div className="min-w-0">
        <Link href={post.permalink} className="font-serif text-xl text-foreground hover:text-primary">
          {post.title}
        </Link>
        <p className="mt-1 text-sm text-muted">
          {post.author_display_name} · {post.blog_slug}
          {post.published_at && <> · {formatDate(post.published_at)}</>}
          {post.category && <> · {post.category.name}</>}
        </p>
        <p className="mt-2 text-sm leading-relaxed text-foreground/80">{excerpt(post.content, 180)}</p>
        {post.tags.length > 0 && (
          <div className="mt-2">
            <TagPills tags={post.tags} />
          </div>
        )}
      </div>
    </article>
  );
}
