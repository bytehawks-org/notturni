import type { Metadata } from "next";
import Link from "next/link";

import { FeedPostCard } from "@/components/FeedPostCard";
import { SiteHeader } from "@/components/shell/SiteHeader";
import { SiteFooter } from "@/components/shell/SiteFooter";
import { BlogDirectoryList } from "@/components/home/BlogDirectory";
import { Manifesto } from "@/components/home/Manifesto";
import { TrendingTags } from "@/components/home/TrendingTags";
import { FilterChip } from "@/components/ui/Pill";
import { getPublicFeed, getTrendingTags, getPublicBlogs } from "@/lib/server-api";

export const metadata: Metadata = { alternates: { canonical: "/" }, openGraph: { type: "website", url: "/" } };

/** Same data as before (feed + trending, tag/category filters) plus the blog directory sidebar. Mockups 4a / 4b. */
export default async function Home({ searchParams }: { searchParams: Promise<{ tag?: string; category?: string; locale?: string }> }) {
  const { tag, category, locale } = await searchParams;
  const [posts, trending, blogs] = await Promise.all([
    getPublicFeed({ limit: 20, tag, category, locale }).catch(() => []),
    getTrendingTags({ days: 7, limit: 8 }).catch(() => []),
    getPublicBlogs({ limit: 5, sort: "active" }).catch(() => []), // new server-api helper → GET /api/v1/blogs?public=1
  ]);
  const filtered = Boolean(tag || category);

  return (
    <>
      <SiteHeader current="latest" />
      <Manifesto />
      <div className="mx-auto grid w-full max-w-[1184px] gap-10 px-5 py-8 lg:grid-cols-[minmax(0,1fr)_320px] lg:gap-16 lg:px-12 lg:py-11">
        <main className="flex min-w-0 flex-col gap-5">
          <TrendingTags tags={trending} active={tag} />
          <div className="mt-2 flex items-center justify-between border-b border-border pb-3">
            <div className="flex gap-6 text-sm text-muted"><span className="-mb-[13px] border-b-2 border-primary pb-3 font-semibold text-foreground">{t("latest")}</span><Link href="/following" className="hover:text-foreground">{t("following")}</Link></div>
            <div className="hidden gap-1.5 md:flex">
              {[["it", "Italiano"], ["en", "English"], ["de", "Deutsch"], ["", t("allLanguages")]].map(([code, label]) => (
                <Link key={code} href={code ? `/?locale=${code}` : "/"}><FilterChip active={(locale ?? "") === code}>{label}</FilterChip></Link>
              ))}
            </div>
          </div>
          {filtered && (
            <div className="flex items-center gap-2.5 text-sm text-muted">
              <span>{tag && <>{t("taggedWith")} <b className="font-semibold text-foreground">#{tag}</b></>}{category && <>{t("inCategory")} <b className="font-semibold text-foreground">{category}</b></>}</span>
              <Link href="/" className="text-[13px]">{t("removeFilter")}</Link>
            </div>
          )}
          {posts.length === 0 ? (
            <p className="py-10 text-center text-sm text-muted">{filtered ? t("noPosts") : t("nothingYet")}</p>
          ) : (
            <div className="flex flex-col">{posts.map((p) => <FeedPostCard key={p.id} post={p} />)}</div>
          )}
          <span className="pt-2 text-center text-[13px] text-muted">{t("showing", { count: posts.length })} · <Link href="/?page=2">{t("older")}</Link> · <Link href="/feed.xml">RSS</Link></span>
        </main>
        <aside className="flex flex-col gap-8 text-sm">
          <BlogDirectoryList blogs={blogs} total={blogs.length} />
        </aside>
      </div>
      <SiteFooter />
    </>
  );
}
