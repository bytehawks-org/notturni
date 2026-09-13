import type { Metadata } from "next";
import { getLocale, getTranslations } from "next-intl/server";
import Link from "next/link";
import { notFound } from "next/navigation";

import { BlogPageShell } from "@/components/blog/BlogPageShell";
import { BlogHeaderActions } from "@/components/blog/PostHeaderActions";
import { BlogHeader } from "@/components/shell/BlogHeader";
import { formatDate } from "@/lib/format";
import { getBlogLinksBibliography, getPublicBlog, getPublicBlogConfig } from "@/lib/server-api";
import type { LinkBibliographyEntry } from "@/lib/types";

interface PageParams {
  blogSlug: string;
}

export async function generateMetadata({ params }: { params: Promise<PageParams> }): Promise<Metadata> {
  const { blogSlug } = await params;
  const [blog, t] = await Promise.all([getPublicBlog(blogSlug), getTranslations("BlogNav")]);
  return { title: blog ? `${t("links")} — ${blog.title}` : t("links") };
}

function hostOf(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}

/** /{blog}/link (mockup 2c/3a): tutti gli URL citati nei post pubblicati,
 * raggruppati per sito e ordinati per numero di link. */
export default async function BlogLinksBibliographyPage({ params }: { params: Promise<PageParams> }) {
  const { blogSlug } = await params;
  const [blog, entries, config, t, tl, locale] = await Promise.all([
    getPublicBlog(blogSlug),
    getBlogLinksBibliography(blogSlug),
    getPublicBlogConfig(blogSlug),
    getTranslations("LinksPage"),
    getTranslations("Links"),
    getLocale(),
  ]);
  if (!blog || !entries) notFound();

  const groups = new Map<string, LinkBibliographyEntry[]>();
  for (const entry of entries) {
    const host = hostOf(entry.url);
    groups.set(host, [...(groups.get(host) ?? []), entry]);
  }
  const sortedGroups = [...groups.entries()].sort((a, b) => b[1].length - a[1].length);

  return (
    <BlogPageShell config={config}>
      <BlogHeader slug={blogSlug} name={blog.title} current="links" actions={<BlogHeaderActions slug={blogSlug} />} />
      <main className="mx-auto w-full max-w-[1184px] flex-1 px-5 py-10 lg:px-12 lg:py-14">
        <div className="flex flex-col gap-1.5">
          <h1 className="font-serif text-[34px] font-medium leading-[1.12] tracking-tight md:text-[40px]">{t("title")}</h1>
          <p className="text-muted md:text-base">{t("subtitle")}</p>
        </div>
        {entries.length === 0 ? (
          <p className="py-10 text-center text-sm text-muted">{t("empty")}</p>
        ) : (
          <div className="mt-6 flex flex-col gap-4 md:gap-8">
            {sortedGroups.map(([host, items]) => {
              const postCount = new Set(items.flatMap((i) => i.citations.map((c) => c.permalink))).size;
              return (
                <section
                  key={host}
                  className="flex flex-col gap-2.5 border-t border-border pt-3.5 md:grid md:grid-cols-[220px_minmax(0,1fr)] md:gap-7 md:pt-6"
                >
                  <div className="flex items-baseline justify-between md:flex-col md:items-start md:gap-1">
                    <h2 className="font-serif text-[17px] md:text-[19px]">{host}</h2>
                    <span className="text-xs text-muted md:text-[13px]">
                      {tl("count", { count: items.length })}
                      {postCount > 1 && ` · ${tl("posts", { count: postCount })}`}
                    </span>
                  </div>
                  <ul className="flex flex-col gap-2.5 md:gap-3.5">
                    {items.map((entry) => {
                      const first = entry.citations[0];
                      return (
                        <li key={entry.url} className="flex min-w-0 flex-col gap-0.5 md:grid md:grid-cols-[minmax(0,1fr)_170px] md:gap-4">
                          <div className="flex min-w-0 flex-col gap-0.5">
                            <a
                              href={entry.url}
                              target="_blank"
                              rel="noopener noreferrer nofollow"
                              className="text-[15px] font-medium text-foreground no-underline hover:text-primary md:text-base"
                            >
                              {entry.link_text || entry.url}
                            </a>
                            <span className="hidden truncate font-mono text-[13px] text-muted md:block">{entry.url}</span>
                          </div>
                          {first && (
                            <span className="text-xs text-muted md:text-right md:text-[13px] md:leading-snug">
                              {tl("in")}{" "}
                              <Link href={first.permalink} className="no-underline hover:underline">
                                {first.post_title}
                              </Link>
                              {first.used_at && (
                                <>
                                  <br className="hidden md:block" /> · {formatDate(first.used_at, locale, { day: "numeric", month: "short", year: "numeric" })}
                                </>
                              )}
                              {entry.citations.length > 1 && ` +${entry.citations.length - 1}`}
                            </span>
                          )}
                        </li>
                      );
                    })}
                  </ul>
                </section>
              );
            })}
          </div>
        )}
      </main>
    </BlogPageShell>
  );
}
