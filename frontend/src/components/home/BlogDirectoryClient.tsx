"use client";

import { useTranslations } from "next-intl";
import Image from "next/image";
import Link from "next/link";

import { SITE_HOST } from "@/lib/site";
import type { Blog } from "@/lib/types";

const hue = (s: string) => `oklch(0.55 0.06 ${[...s].reduce((a, c) => a + c.charCodeAt(0), 0) % 360})`;

/** Variante client della griglia della directory (tab "Blog" del profilo, mockup 3e). */
export function BlogDirectoryGridClient({ blogs }: { blogs: Blog[] }) {
  const t = useTranslations("Directory");
  return (
    <div className="grid gap-5 sm:grid-cols-2">
      {blogs.map((b) => (
        <article key={b.slug} className="flex min-w-0 flex-col gap-3 rounded-[14px] border border-border bg-surface p-[22px]">
          <div className="flex items-center gap-3">
            {b.favicon_url ? (
              <Image
                src={b.favicon_url}
                alt={b.title}
                width={44}
                height={44}
                className="h-11 w-11 shrink-0 rounded-[10px] object-cover"
                unoptimized
              />
            ) : (
              <span
                className="grid h-11 w-11 shrink-0 place-items-center rounded-[10px] font-serif text-lg text-white"
                style={{ background: hue(b.slug) }}
                aria-hidden="true"
              >
                {b.title[0]}
              </span>
            )}
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
          <Link href={`/${b.slug}`} className="mt-auto text-[13px] font-medium no-underline hover:underline">
            {t("follow")}
          </Link>
        </article>
      ))}
    </div>
  );
}
