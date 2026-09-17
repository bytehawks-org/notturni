import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import Link from "next/link";
import { notFound } from "next/navigation";

import { BlogPageShell } from "@/components/blog/BlogPageShell";
import { BlogStateNotice, blogIsOffline } from "@/components/blog/BlogStateNotice";
import { BlogHeaderActions } from "@/components/blog/PostHeaderActions";
import { BlogHeader } from "@/components/shell/BlogHeader";
import { blogBasePath } from "@/lib/blog-path";
import { getPublicBlog, getPublicBlogConfig, getPublicPublications } from "@/lib/server-api";

interface PageParams {
  blogSlug: string;
}

export async function generateMetadata({ params }: { params: Promise<PageParams> }): Promise<Metadata> {
  const { blogSlug } = await params;
  const [blog, t] = await Promise.all([getPublicBlog(blogSlug), getTranslations("BlogNav")]);
  return { title: blog ? `${t("publications")} — ${blog.title}` : t("publications") };
}

/** /{blog}/pub — elenco delle pubblicazioni con capitoli pubblicati (B9). */
export default async function PublicationsPage({ params }: { params: Promise<PageParams> }) {
  const { blogSlug } = await params;
  const [blog, config, publications, t, basePath] = await Promise.all([
    getPublicBlog(blogSlug),
    getPublicBlogConfig(blogSlug),
    getPublicPublications(blogSlug),
    getTranslations("Publication"),
    blogBasePath(blogSlug),
  ]);
  if (!blog) notFound();
  if (blogIsOffline(blog)) {
    return (
      <BlogPageShell config={config}>
        <BlogHeader basePath={basePath} name={blog.title} current="publications" hasPublications />
        <BlogStateNotice blog={blog} />
      </BlogPageShell>
    );
  }
  return (
    <BlogPageShell config={config}>
      <BlogHeader basePath={basePath} name={blog.title} current="publications" hasPublications actions={<BlogHeaderActions slug={blogSlug} />} />
      <main className="mx-auto w-full max-w-[860px] flex-1 px-5 py-10 lg:px-12 lg:py-14">
        <h1 className="font-serif text-[34px] font-medium leading-[1.12] tracking-tight md:text-[40px]">{t("listTitle")}</h1>
        <p className="mt-1.5 text-muted md:text-base">{t("listSubtitle")}</p>
        {publications.length === 0 ? (
          <p className="py-10 text-center text-sm text-muted">{t("none")}</p>
        ) : (
          <ul className="mt-8 flex flex-col">
            {publications.map((p) => (
              <li key={p.id} className="border-b border-border py-5">
                <Link href={`${basePath}/pub/${p.name}`} className="font-serif text-2xl text-foreground no-underline hover:text-primary">
                  {p.title}
                </Link>
                {p.description && <p className="mt-1 text-[15px] text-muted">{p.description}</p>}
                <p className="mt-2 text-[13px] text-muted">{t("chaptersOf", { published: p.chapters_published, total: p.chapters_total })}</p>
              </li>
            ))}
          </ul>
        )}
      </main>
    </BlogPageShell>
  );
}
