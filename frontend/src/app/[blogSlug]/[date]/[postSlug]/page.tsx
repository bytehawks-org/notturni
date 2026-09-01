import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";

import { SiteHeader } from "@/components/SiteHeader";
import { TagPills } from "@/components/TagPills";
import { excerpt, renderMarkdown } from "@/lib/markdown";
import { getPublicPostByPermalink } from "@/lib/server-api";

interface PageParams {
  blogSlug: string;
  date: string;
  postSlug: string;
}

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString("it-IT", { day: "numeric", month: "long", year: "numeric" });
}

export async function generateMetadata({ params }: { params: Promise<PageParams> }): Promise<Metadata> {
  const { blogSlug, date, postSlug } = await params;
  const post = await getPublicPostByPermalink(blogSlug, date, postSlug);
  if (!post) return {};
  return {
    title: post.title,
    description: excerpt(post.content),
  };
}

export default async function PublicPostPage({ params }: { params: Promise<PageParams> }) {
  const { blogSlug, date, postSlug } = await params;
  const post = await getPublicPostByPermalink(blogSlug, date, postSlug);
  if (!post) notFound();

  const html = renderMarkdown(post.content, {
    mentions: post.mentions_enabled,
    notes: post.notes,
  });

  return (
    <div className="flex flex-1 flex-col">
      <SiteHeader />
      <main className="mx-auto w-full max-w-2xl flex-1 px-6 py-12">
        <Link href={`/${blogSlug}`} className="text-sm text-muted hover:text-foreground">
          ← {blogSlug}
        </Link>

        <h1 className="mt-6 font-serif text-5xl font-semibold leading-tight text-foreground">{post.title}</h1>
        <p className="mt-4 text-sm text-muted">
          {post.author_display_name}
          {post.published_at && <> · {formatDate(post.published_at)}</>}
          {post.category && (
            <>
              {" · "}
              <Link href={`/?category=${encodeURIComponent(post.category.slug)}`} className="hover:text-primary">
                {post.category.name}
              </Link>
            </>
          )}
        </p>

        {post.cover_image_url && post.cover_image_is_sensitive && (
          <div className="sensitive-image-wrapper mt-8 aspect-[16/9] w-full overflow-hidden rounded-xl">
            <input type="checkbox" className="sensitive-image-toggle" aria-label="Rivela l'immagine di copertina" />
            {/* eslint-disable-next-line @next/next/no-img-element -- URL S3/MinIO esterno */}
            <img src={post.cover_image_url} alt="" className="h-full w-full object-cover" />
            <span className="sensitive-image-overlay">Contenuto sensibile — clicca per vedere</span>
          </div>
        )}
        {post.cover_image_url && !post.cover_image_is_sensitive && (
          // eslint-disable-next-line @next/next/no-img-element -- URL S3/MinIO esterno
          <img
            src={post.cover_image_url}
            alt=""
            className="mt-8 aspect-[16/9] w-full rounded-xl object-cover"
          />
        )}

        <div className="notturni-prose mt-10 text-lg leading-relaxed" dangerouslySetInnerHTML={{ __html: html }} />

        {post.notes.length > 0 && (
          <p className="mt-6 text-sm">
            <Link href={`/${blogSlug}/bibliografia`} className="text-primary hover:underline">
              Tutte le note di questo blog →
            </Link>
          </p>
        )}

        {post.tags.length > 0 && (
          <div className="mt-10">
            <TagPills tags={post.tags} label="Tag:" />
          </div>
        )}
      </main>
    </div>
  );
}
