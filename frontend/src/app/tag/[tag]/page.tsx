import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import Link from "next/link";

import { FeedPostCard } from "@/components/FeedPostCard";
import { SiteHeader } from "@/components/SiteHeader";
import { SiteFooter } from "@/components/shell/SiteFooter";
import { getPublicFeed } from "@/lib/server-api";

const PAGE_SIZE = 20;

interface PageProps {
  params: Promise<{ tag: string }>;
  searchParams: Promise<{ page?: string }>;
}

export async function generateMetadata({ params }: PageProps): Promise<Metadata> {
  const tag = decodeURIComponent((await params).tag);
  return { title: `#${tag}`, alternates: { canonical: `/tag/${encodeURIComponent(tag)}` } };
}

/** Elenco paginato dei post pubblici con un dato tag (campo dedicato o #hashtag nel testo).
 * Si chiede un post in più della pagina per sapere se ne esiste una successiva. */
export default async function TagPage({ params, searchParams }: PageProps) {
  const tag = decodeURIComponent((await params).tag);
  const page = Math.max(1, Math.floor(Number((await searchParams).page)) || 1);
  const t = await getTranslations("TagPage");
  const fetched = await getPublicFeed({ tag, limit: PAGE_SIZE + 1, offset: (page - 1) * PAGE_SIZE }).catch(() => []);
  const posts = fetched.slice(0, PAGE_SIZE);
  const hasNext = fetched.length > PAGE_SIZE;
  const href = (n: number) => `/tag/${encodeURIComponent(tag)}${n > 1 ? `?page=${n}` : ""}`;

  return (
    <>
      <SiteHeader />
      <main className="mx-auto flex w-full max-w-[760px] flex-1 flex-col gap-6 px-5 py-10 lg:py-14">
        <header className="flex flex-col gap-1">
          <span className="text-sm text-muted">{t("title")}</span>
          <h1 className="font-serif text-[34px] font-medium leading-tight text-foreground">#{tag}</h1>
        </header>
        {posts.length === 0 ? (
          <p className="py-10 text-center text-sm text-muted">{t("noPosts")}</p>
        ) : (
          <div className="flex flex-col">
            {posts.map((post) => (
              <FeedPostCard key={post.id} post={post} />
            ))}
          </div>
        )}
        {(page > 1 || hasNext) && (
          <nav className="flex items-center justify-between border-t border-border pt-4 text-sm">
            {page > 1 ? (
              <Link href={href(page - 1)} className="text-primary no-underline hover:underline">
                {t("previous")}
              </Link>
            ) : (
              <span />
            )}
            <span className="text-muted">{t("page", { page })}</span>
            {hasNext ? (
              <Link href={href(page + 1)} className="text-primary no-underline hover:underline">
                {t("next")}
              </Link>
            ) : (
              <span />
            )}
          </nav>
        )}
      </main>
      <SiteFooter />
    </>
  );
}
