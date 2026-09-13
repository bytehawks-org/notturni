import { getTranslations } from "next-intl/server";
import Link from "next/link";

import { SITE_HOST } from "@/lib/site";
import type { Blog } from "@/lib/types";

const hue = (s: string) => `oklch(0.55 0.06 ${[...s].reduce((a, c) => a + c.charCodeAt(0), 0) % 360})`;

function Avatar({ blog, size = 44 }: { blog: Blog; size?: number }) {
  return (
    <span
      className="grid shrink-0 place-items-center rounded-[10px] font-serif text-white"
      style={{ width: size, height: size, background: hue(blog.slug), fontSize: size * 0.43 }}
      aria-hidden="true"
    >
      {blog.title[0]}
    </span>
  );
}

/** Elenco compatto nella sidebar della home (mockup 4a). Il backend non
 * espone ancora conteggi di post/follower per blog (blocco B1): la riga
 * meta mostra solo host e lingua principale. */
export async function BlogDirectoryList({ blogs, total }: { blogs: Blog[]; total: number }) {
  const t = await getTranslations("Directory");
  if (!blogs.length) return null;
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-baseline justify-between">
        <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("title", { total })}</span>
        <Link href="/blogs" className="text-[13px] no-underline hover:underline">
          {t("allBlogs")}
        </Link>
      </div>
      <ul className="flex flex-col">
        {blogs.map((b) => (
          <li key={b.slug} className="grid grid-cols-[36px_minmax(0,1fr)_auto] items-center gap-3 border-b border-border py-2.5">
            <Avatar blog={b} size={36} />
            <div className="flex min-w-0 flex-col leading-snug">
              <Link href={`/${b.slug}`} className="truncate font-semibold text-foreground no-underline hover:text-primary">
                {b.title}
              </Link>
              <span className="truncate text-xs text-muted">
                {b.slug}.{SITE_HOST} · {b.default_locale.toUpperCase()}
              </span>
            </div>
            <Link href={`/${b.slug}`} className="text-[13px] font-medium no-underline hover:underline">
              {t("follow")}
            </Link>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** Griglia di card per /blogs (mockup 4c). */
export async function BlogDirectoryGrid({ blogs }: { blogs: Blog[] }) {
  const t = await getTranslations("Directory");
  if (!blogs.length) return <p className="py-10 text-center text-sm text-muted">{t("noMatch")}</p>;
  return (
    <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
      {blogs.map((b) => (
        <article key={b.slug} className="flex min-w-0 flex-col gap-3 rounded-[14px] border border-border bg-surface p-[22px]">
          <div className="flex items-center gap-3">
            <Avatar blog={b} />
            <div className="flex min-w-0 flex-col leading-tight">
              <Link href={`/${b.slug}`} className="truncate font-serif text-[19px] text-foreground no-underline hover:text-primary">
                {b.title}
              </Link>
              <span className="font-mono text-xs text-muted">
                {b.slug}.{SITE_HOST}
              </span>
            </div>
          </div>
          {(b.subtitle || b.description) && <p className="text-sm leading-relaxed text-muted">{b.subtitle ?? b.description}</p>}
          <div className="mt-auto flex items-center justify-between pt-1.5 text-[13px] text-muted">
            <span>{b.default_locale.toUpperCase()}</span>
            <Link href={`/${b.slug}`} className="font-medium no-underline hover:underline">
              {t("follow")}
            </Link>
          </div>
        </article>
      ))}
    </div>
  );
}
