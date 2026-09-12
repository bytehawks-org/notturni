import Link from "next/link";

import type { Blog } from "@/lib/types";

const hue = (s: string) => `oklch(0.55 0.06 ${[...s].reduce((a, c) => a + c.charCodeAt(0), 0) % 360})`;

function Avatar({ blog, size = 44 }: { blog: Blog; size?: number }) {
  return (
    <span
      className="grid place-items-center rounded-[10px] font-serif text-white"
      style={{ width: size, height: size, background: hue(blog.slug), fontSize: size * 0.43 }}
      aria-hidden="true"
    >
      {blog.title[0]}
    </span>
  );
}

/** Griglia di card per /blogs (todo/UX_REDESIGN.md, mockup 4c). */
export function BlogDirectoryGrid({ blogs }: { blogs: Blog[] }) {
  if (!blogs.length) return <p className="py-10 text-center text-sm text-muted">Nessun blog trovato.</p>;
  return (
    <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
      {blogs.map((b) => (
        <Link
          key={b.slug}
          href={`/${b.slug}`}
          className="flex min-w-0 flex-col gap-3 rounded-[14px] border border-border bg-surface p-[22px] no-underline"
        >
          <div className="flex items-center gap-3">
            <Avatar blog={b} />
            <div className="flex min-w-0 flex-col leading-tight">
              <span className="truncate font-serif text-[19px] text-foreground">{b.title}</span>
              <span className="font-mono text-xs text-muted">/{b.slug}</span>
            </div>
          </div>
          {(b.subtitle || b.description) && (
            <p className="text-sm leading-relaxed text-muted">{b.subtitle ?? b.description}</p>
          )}
        </Link>
      ))}
    </div>
  );
}
