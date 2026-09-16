import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import Link from "next/link";
import { notFound } from "next/navigation";
import type { ReactNode } from "react";

import { BlogPageShell } from "@/components/blog/BlogPageShell";
import { BlogStateNotice, blogIsOffline } from "@/components/blog/BlogStateNotice";
import { BlogHeaderActions } from "@/components/blog/PostHeaderActions";
import { BlogHeader } from "@/components/shell/BlogHeader";
import { FilterChip } from "@/components/ui/Pill";
import { renderNoteInline } from "@/lib/markdown";
import { getBlogBibliography, getPublicBlog, getPublicBlogConfig, getPublicPublications } from "@/lib/server-api";

interface PageParams {
  blogSlug: string;
}

export async function generateMetadata({ params }: { params: Promise<PageParams> }): Promise<Metadata> {
  const { blogSlug } = await params;
  const [blog, t] = await Promise.all([getPublicBlog(blogSlug), getTranslations("BlogNav")]);
  if (!blog) return { title: t("bibliography") };
  const title = `${t("bibliography")} — ${blog.title}`;
  return {
    title,
    alternates: { canonical: `/${blogSlug}/bibliografia` },
    robots: blog.search_indexing_enabled ? undefined : { index: false, follow: false },
    openGraph: { title, type: "website", url: `/${blogSlug}/bibliografia` },
  };
}

/** /{blog}/bibliografia (mockup 2a/3a): note deduplicate dei post pubblicati,
 * numerate, con i post che le citano. Il filtro per tipo (libro/articolo/web)
 * richiede le note come entità con `kind` (blocco B8): qui solo l'ordinamento. */
export default async function BlogBibliographyPage({
  params,
  searchParams,
}: {
  params: Promise<PageParams>;
  searchParams: Promise<{ sort?: string; kind?: string }>;
}) {
  const [{ blogSlug }, { sort, kind }] = await Promise.all([params, searchParams]);
  const [blog, entries, config, t] = await Promise.all([
    getPublicBlog(blogSlug),
    getBlogBibliography(blogSlug),
    getPublicBlogConfig(blogSlug),
    getTranslations("BibliographyPage"),
  ]);
  if (!blog) notFound();
  const hasPublications = (await getPublicPublications(blogSlug).catch(() => [])).length > 0;
  if (blogIsOffline(blog)) {
    return (
      <BlogPageShell config={config}>
        <BlogHeader slug={blogSlug} name={blog.title} hasPublications={hasPublications} current="bibliography" />
        <BlogStateNotice blog={blog} />
      </BlogPageShell>
    );
  }
  if (!entries) notFound();
  const tb = await getTranslations("Bibliography");
  const filtered = kind ? entries.filter((e) => e.kind === kind) : entries;
  const sorted = sort === "cited" ? [...filtered].sort((a, b) => b.citations.length - a.citations.length) : filtered;
  const kinds = (["book", "article", "web", "note"] as const).filter((k) => entries.some((e) => e.kind === k));
  const href = (next: { kind?: string; sort?: string }) => {
    const p = new URLSearchParams();
    const k = next.kind ?? kind;
    const so = next.sort ?? sort;
    if (k) p.set("kind", k);
    if (so) p.set("sort", so);
    const qs = p.toString();
    return `/${blogSlug}/bibliografia${qs ? `?${qs}` : ""}`;
  };
  const postCount = new Set(entries.flatMap((e) => e.citations.map((c) => c.permalink))).size;

  return (
    <BlogPageShell config={config}>
      <BlogHeader slug={blogSlug} name={blog.title} hasPublications={hasPublications} current="bibliography" actions={<BlogHeaderActions slug={blogSlug} />} />
      <main className="mx-auto w-full max-w-[1184px] flex-1 px-5 py-10 lg:px-12 lg:py-14">
        <div className="flex flex-col gap-1.5">
          <h1 className="font-serif text-[34px] font-medium leading-[1.12] tracking-tight md:text-[40px]">{t("title")}</h1>
          <p className="text-muted md:text-base">{t("subtitle", { notes: entries.length, posts: postCount })}</p>
        </div>
        <div className="mt-6 flex flex-wrap items-center justify-between gap-3 border-b border-border pb-3">
          <div className="flex flex-wrap gap-1.5">
            <Link href={href({ kind: "" })} className="no-underline">
              <FilterChip active={!kind}>{t("all", { count: entries.length })}</FilterChip>
            </Link>
            {kinds.map((k) => (
              <Link key={k} href={href({ kind: k })} className="no-underline">
                <FilterChip active={kind === k}>
                  {tb(`kind.${k}`)} · {entries.filter((e) => e.kind === k).length}
                </FilterChip>
              </Link>
            ))}
          </div>
          <div className="flex gap-1.5">
            <Link href={href({ sort: "" })} className="no-underline">
              <FilterChip active={sort !== "cited"}>{t("sortFirst")}</FilterChip>
            </Link>
            <Link href={href({ sort: "cited" })} className="no-underline">
              <FilterChip active={sort === "cited"}>{t("sortCited")}</FilterChip>
            </Link>
          </div>
        </div>
        {entries.length === 0 ? (
          <p className="py-10 text-center text-sm text-muted">{t("empty")}</p>
        ) : (
          <ol className="flex flex-col">
            {sorted.map((entry, i) => (
              <li
                key={i}
                className="grid grid-cols-[30px_minmax(0,1fr)] gap-2.5 border-b border-border py-3.5 md:grid-cols-[44px_minmax(0,1fr)_200px] md:gap-5 md:py-5"
              >
                <span className="pt-0.5 font-serif text-[15px] text-muted">{String(i + 1).padStart(2, "0")}</span>
                <div className="flex min-w-0 flex-col gap-2">
                  <div
                    className="notturni-prose text-[15px] leading-[1.45] md:text-[17px]"
                    dangerouslySetInnerHTML={{ __html: renderNoteInline(entry.content) }}
                  />
                  {(entry.author || entry.title || entry.source || entry.issued || entry.page || entry.isbn || entry.doi) && (
                    <p className="text-[13px] text-muted">
                      {(
                        [
                          entry.author,
                          entry.title ? <em key="title">{entry.title}</em> : null,
                          entry.source,
                          entry.issued,
                          entry.page ? tb("page", { page: entry.page }) : null,
                          entry.isbn ? tb("isbn", { isbn: entry.isbn }) : null,
                          entry.doi ? (
                            <a
                              key="doi"
                              href={`https://doi.org/${entry.doi}`}
                              target="_blank"
                              rel="noopener noreferrer nofollow"
                              className="text-primary no-underline hover:underline"
                            >
                              doi.org/{entry.doi}
                            </a>
                          ) : null,
                        ] as (ReactNode | null)[]
                      )
                        .filter((part): part is ReactNode => part !== null && part !== "")
                        .reduce<ReactNode[]>((acc, part, i) => (i > 0 ? [...acc, " · ", part] : [part]), [])}
                    </p>
                  )}
                  <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[13px] text-muted">
                    {entry.kind && (
                      <span className="rounded border border-border px-1.5 py-0.5 font-mono text-[11px] uppercase tracking-[.06em]">{tb(`kind.${entry.kind}`)}</span>
                    )}
                    {entry.url && (
                      <a href={entry.url} target="_blank" rel="noopener noreferrer nofollow" className="truncate text-primary no-underline hover:underline">
                        {entry.url.replace(/^https?:\/\//, "")}
                      </a>
                    )}
                    <span>{tb("citedIn")}</span>
                    {entry.citations.map((c, j) => (
                      <Link key={`${c.permalink}-${c.idx}`} href={`${c.permalink}#fn-${c.idx}`} className="no-underline hover:underline">
                        {c.post_title}
                        {c.locale !== blog.default_locale ? ` (${c.locale})` : ""}
                        {j < entry.citations.length - 1 ? "," : ""}
                      </Link>
                    ))}
                  </div>
                </div>
                <div className="hidden flex-col items-end gap-1 text-right text-[13px] text-muted md:flex">
                  <span className="font-semibold text-foreground">{tb("citations", { count: entry.citations.length })}</span>
                </div>
              </li>
            ))}
          </ol>
        )}
      </main>
    </BlogPageShell>
  );
}
