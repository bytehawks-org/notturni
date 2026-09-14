import { useTranslations } from "next-intl";
import Link from "next/link";
import type { PublicBlog } from "@/lib/types";

const hue = (s: string) => `oklch(0.55 0.06 ${[...s].reduce((a, c) => a + c.charCodeAt(0), 0) % 360})`;

function Avatar({ blog, size = 36 }: { blog: PublicBlog; size?: number }) {
  return blog.avatar_url
    ? /* eslint-disable-next-line @next/next/no-img-element */ <img src={blog.avatar_url} alt="" width={size} height={size} className="rounded-[10px] object-cover" />
    : <span className="grid place-items-center rounded-[10px] font-serif text-white" style={{ width: size, height: size, background: hue(blog.slug), fontSize: size * 0.43 }}>{blog.title[0]}</span>;
}
const meta = (b: PublicBlog, t: (k: string, v?: Record<string, number>) => string) => [b.languages?.join(" · "), t("postsCount", { count: b.post_count })].filter(Boolean).join(" · ");

/** Sidebar list on the home (4a). */
export function BlogDirectoryList({ blogs, total }: { blogs: PublicBlog[]; total: number }) {
  const t = useTranslations("Directory");
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-baseline justify-between"><span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("title", { total })}</span><Link href="/blogs" className="text-[13px]">{t("allBlogs")}</Link></div>
      <ul className="flex flex-col">
        {blogs.map((b) => (
          <li key={b.slug} className="grid grid-cols-[36px_minmax(0,1fr)_auto] items-center gap-3 border-b border-border py-2.5">
            <Avatar blog={b} />
            <div className="flex min-w-0 flex-col leading-snug"><Link href={`/${b.slug}`} className="font-semibold text-foreground no-underline">{b.title}</Link><span className="truncate text-xs text-muted">{b.slug}.notturni.eu · {meta(b, t)}</span></div>
            <Link href={`/${b.slug}?follow=1`} className="text-[13px] font-medium">{t("follow")}</Link>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** Card grid on /blogs (4c). */
export function BlogDirectoryGrid({ blogs }: { blogs: PublicBlog[] }) {
  const t = useTranslations("Directory");
  if (!blogs.length) return <p className="py-10 text-center text-sm text-muted">{t("noMatch")}</p>;
  return (
    <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
      {blogs.map((b) => (
        <article key={b.slug} className="flex min-w-0 flex-col gap-3 rounded-[14px] border border-border bg-surface p-[22px]">
          <div className="flex items-center gap-3"><Avatar blog={b} size={44} /><div className="flex min-w-0 flex-col leading-tight"><Link href={`/${b.slug}`} className="font-serif text-[19px] text-foreground no-underline">{b.title}</Link><span className="font-mono text-xs text-muted">{b.slug}.notturni.eu</span></div></div>
          {(b.subtitle || b.description) && <p className="text-sm leading-relaxed text-muted">{b.subtitle ?? b.description}</p>}
          <div className="mt-auto flex items-center justify-between pt-1.5 text-[13px] text-muted"><span>{meta(b, t)}{b.follower_count != null && ` · ${t("followers", { count: b.follower_count })}`}</span><Link href={`/${b.slug}?follow=1`} className="font-medium">{t("follow")}</Link></div>
        </article>
      ))}
    </div>
  );
}
