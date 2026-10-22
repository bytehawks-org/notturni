import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import Link from "next/link";
import { notFound } from "next/navigation";

import { BlogPageShell } from "@/components/blog/BlogPageShell";
import { BlogStateNotice, blogIsOffline } from "@/components/blog/BlogStateNotice";
import { BlogHeaderActions } from "@/components/blog/PostHeaderActions";
import { PublicationIndex } from "@/components/publications/PublicationIndex";
import { BlogHeader } from "@/components/shell/BlogHeader";
import { Button } from "@/components/ui/Button";
import { blogLinks } from "@/lib/blog-path";
import { getPublicBlog, getPublicBlogConfig, getPublicPublication } from "@/lib/server-api";

interface PageParams {
  blogSlug: string;
  name: string;
}

export async function generateMetadata({ params }: { params: Promise<PageParams> }): Promise<Metadata> {
  const { blogSlug, name } = await params;
  const [blog, pub] = await Promise.all([getPublicBlog(blogSlug), getPublicPublication(blogSlug, name)]);
  if (!blog || !pub) return {};
  return {
    title: `${pub.title} — ${blog.title}`,
    description: pub.description ?? undefined,
    alternates: { canonical: `/${blogSlug}/pub/${name}` },
  };
}

/** /{blog}/pub/{name} — indice della pubblicazione (mockup 2d). */
export default async function PublicationPage({ params }: { params: Promise<PageParams> }) {
  const { blogSlug, name } = await params;
  const [blog, config, pub, t, links] = await Promise.all([
    getPublicBlog(blogSlug),
    getPublicBlogConfig(blogSlug),
    getPublicPublication(blogSlug, name),
    getTranslations("Publication"),
    blogLinks(blogSlug),
  ]);
  if (!blog) notFound();
  if (blogIsOffline(blog)) {
    return (
      <BlogPageShell config={config}>
        <BlogHeader basePath={links.basePath} name={blog.title} current="publications" hasPublications />
        <BlogStateNotice blog={blog} />
      </BlogPageShell>
    );
  }
  if (!pub) notFound();
  const totalMinutes = pub.chapters.reduce((sum, c) => sum + c.reading_minutes, 0);
  const first = pub.chapters[0];
  return (
    <BlogPageShell config={config}>
      <BlogHeader basePath={links.basePath} name={blog.title} current="publications" hasPublications actions={<BlogHeaderActions slug={blogSlug} />} />
      <main className="mx-auto w-full max-w-[860px] flex-1 px-5 py-10 lg:px-12 lg:py-14">
        <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("label")}</span>
        <h1 className="mt-2 font-serif text-[34px] font-medium leading-[1.12] tracking-tight md:text-[44px]">{pub.title}</h1>
        {pub.description && <p className="mt-3 max-w-[640px] text-lg leading-relaxed text-muted">{pub.description}</p>}
        <div className="mt-4 flex flex-wrap items-center gap-4 text-sm text-muted">
          <span>{t("chaptersOf", { published: pub.chapters_published, total: pub.chapters_total })}</span>
          <span>· {t("minutes", { min: totalMinutes })}</span>
          {first && (
            <Link href={links.fromPermalink(first.permalink)} className="no-underline">
              <Button size="sm">{t("startReading")}</Button>
            </Link>
          )}
        </div>
        <div className="mt-10">
          <PublicationIndex chapters={pub.chapters} resolvePermalink={links.fromPermalink} />
        </div>
      </main>
    </BlogPageShell>
  );
}
