import type { Metadata } from "next";
import { notFound } from "next/navigation";

import { FeedPostCard } from "@/components/FeedPostCard";
import { SiteHeader } from "@/components/SiteHeader";
import { getPublicBlog, getPublicBlogPosts } from "@/lib/server-api";

interface PageParams {
  blogSlug: string;
}

export async function generateMetadata({ params }: { params: Promise<PageParams> }): Promise<Metadata> {
  const { blogSlug } = await params;
  const blog = await getPublicBlog(blogSlug);
  if (!blog) return {};
  return {
    title: blog.title,
    description: blog.description ?? blog.subtitle ?? undefined,
  };
}

export default async function BlogHomePage({ params }: { params: Promise<PageParams> }) {
  const { blogSlug } = await params;
  const [blog, posts] = await Promise.all([getPublicBlog(blogSlug), getPublicBlogPosts(blogSlug)]);
  if (!blog || !posts) notFound();

  return (
    <div className="flex flex-1 flex-col">
      <SiteHeader />
      <main className="mx-auto w-full max-w-2xl flex-1 px-6 py-12">
        <h1 className="font-serif text-4xl font-semibold leading-tight text-foreground">{blog.title}</h1>
        {blog.subtitle && <p className="mt-2 text-lg text-muted">{blog.subtitle}</p>}
        {blog.description && <p className="mt-4 text-sm leading-relaxed text-foreground/80">{blog.description}</p>}

        <div className="mt-10">
          {posts.length === 0 ? (
            <p className="text-sm text-muted">Ancora nessun post pubblicato.</p>
          ) : (
            posts.map((post) => <FeedPostCard key={post.id} post={post} />)
          )}
        </div>
      </main>
    </div>
  );
}
