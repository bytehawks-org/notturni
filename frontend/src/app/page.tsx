import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import Link from "next/link";

import { FeedPostCard } from "@/components/FeedPostCard";
import { BlogDirectoryList } from "@/components/home/BlogDirectory";
import { FollowingFeed } from "@/components/home/FollowingFeed";
import { Manifesto } from "@/components/home/Manifesto";
import { TrendingTags } from "@/components/home/TrendingTags";
import { NewsletterSignup } from "@/components/NewsletterSignup";
import { SiteHeader } from "@/components/SiteHeader";
import { SiteFooter } from "@/components/shell/SiteFooter";
import { FilterChip } from "@/components/ui/Pill";
import { languageName } from "@/lib/languages";
import {
  getPublicBlogs,
  getPublicFeed,
  getTrendingTags,
} from "@/lib/server-api";

export const metadata: Metadata = {
  alternates: {
    canonical: "/",
    types: { "application/rss+xml": "/feed.xml", "application/atom+xml": "/atom.xml" },
  },
  openGraph: { type: "website", url: "/" },
};

/** Lingue dei contenuti proposte come filtro rapido del feed (mockup 4a);
 * il filtro `locale` del backend accetta qualunque codice a 2 lettere. */
const FEED_LOCALES = ["it", "en", "de", "fr"];

/** Home di piattaforma (mockup 4a desktop / 4b mobile): manifesto, tendenze,
 * feed con filtri tag/categoria/lingua, tab "Seguiti" (client, richiede
 * sessione), directory dei blog in sidebar. "Pubblicazioni in corso" arriva con B9. */
export default async function Home({
  searchParams,
}: {
  searchParams: Promise<{
    tag?: string;
    category?: string;
    locale?: string;
    tab?: string;
  }>;
}) {
  const { tag, category, locale, tab } = await searchParams;
  const followingTab = tab === "following";
  const [posts, trending, blogs, t, tHome] = await Promise.all([
    getPublicFeed({ limit: 20, tag, category, locale }).catch(() => []),
    getTrendingTags({ days: 7, limit: 8 }).catch(() => []),
    getPublicBlogs({ limit: 6, sort: "active" }).catch(() => []),
    getTranslations("HomePage"),
    getTranslations("Home"),
  ]);
  const filtered = Boolean(tag || category);
  const blogTitles = new Map(blogs.map((b) => [b.slug, b.title]));

  const localeHref = (code: string) => {
    const params = new URLSearchParams();
    if (tag) params.set("tag", tag);
    if (category) params.set("category", category);
    if (code) params.set("locale", code);
    const qs = params.toString();
    return qs ? `/?${qs}` : "/";
  };

  return (
    <>
      <SiteHeader />
      <Manifesto />
      <div className="mx-auto grid w-full max-w-[1184px] gap-10 px-5 py-8 lg:grid-cols-[minmax(0,1fr)_320px] lg:gap-16 lg:px-12 lg:py-11">
        <main className="flex min-w-0 flex-col gap-5">
          <TrendingTags tags={trending} active={tag} />
          <div className="mt-2 flex flex-wrap items-center justify-between gap-3 border-b border-border pb-3">
            <div className="flex gap-6 text-sm text-muted">
              <Link
                href="/"
                className={`-mb-[13px] pb-3 no-underline ${followingTab ? "hover:text-foreground" : "border-b-2 border-primary font-semibold text-foreground"}`}
              >
                {t("latest")}
              </Link>
              <Link
                href="/?tab=following"
                className={`-mb-[13px] pb-3 no-underline ${followingTab ? "border-b-2 border-primary font-semibold text-foreground" : "hover:text-foreground"}`}
              >
                {t("following")}
              </Link>
            </div>
            <div className="flex gap-1.5 overflow-x-auto">
              {FEED_LOCALES.map((code) => (
                <Link
                  key={code}
                  href={localeHref(code)}
                  className="no-underline"
                >
                  <FilterChip active={locale === code}>
                    {languageName(code, code)}
                  </FilterChip>
                </Link>
              ))}
              <Link href={localeHref("")} className="no-underline">
                <FilterChip active={!locale}>{t("allLanguages")}</FilterChip>
              </Link>
              <Link href="/feed.xml" className="ml-1 text-[13px] text-muted no-underline hover:text-foreground">
                {t("rssLink")}
              </Link>
            </div>
          </div>
          {followingTab ? (
            <FollowingFeed />
          ) : (
            <>
              {filtered && (
                <div className="flex items-center gap-2.5 text-sm text-muted">
                  <span>
                    {tag && (
                      <>
                        {t("taggedWith")}{" "}
                        <b className="font-semibold text-foreground">#{tag}</b>
                      </>
                    )}
                    {category && (
                      <>
                        {t("inCategory")}{" "}
                        <b className="font-semibold text-foreground">
                          {category}
                        </b>
                      </>
                    )}
                  </span>
                  <Link
                    href="/"
                    className="text-[13px] no-underline hover:underline"
                  >
                    {t("removeFilter")}
                  </Link>
                </div>
              )}
              {posts.length === 0 ? (
                <p className="py-10 text-center text-sm text-muted">
                  {filtered ? t("noPosts") : t("nothingYet")}
                </p>
              ) : (
                <div className="flex flex-col">
                  {posts.map((post) => (
                    <FeedPostCard
                      key={post.id}
                      post={post}
                      blogTitle={blogTitles.get(post.blog_slug)}
                    />
                  ))}
                </div>
              )}
              {posts.length > 0 && (
                <span className="pt-2 text-center text-[13px] text-muted">
                  {t("showing", { count: posts.length })}
                </span>
              )}
            </>
          )}
        </main>
        <aside className="flex flex-col gap-8 text-sm">
          <BlogDirectoryList blogs={blogs} total={blogs.length} />
          <div className="flex flex-col gap-2 rounded-xl border border-border bg-surface px-[18px] py-4 text-[13px] leading-relaxed text-muted">
            <span className="font-serif text-[17px] text-foreground">
              {tHome("pillars.portability.title")}
            </span>
            <span>{tHome("pillars.portability.body")}</span>
            <Link
              href="/register"
              className="font-medium text-primary no-underline hover:underline"
            >
              {tHome("startBlog")} →
            </Link>
          </div>
          <NewsletterSignup variant="platform" />
        </aside>
      </div>
      <SiteFooter />
    </>
  );
}
