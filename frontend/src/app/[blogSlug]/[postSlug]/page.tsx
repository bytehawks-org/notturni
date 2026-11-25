import type { Metadata, ResolvingMetadata } from "next";
import { permanentRedirect } from "next/navigation";

import { PublicPostView, generatePostMetadata, type PageParams } from "@/components/blog/PublicPostView";
import { getPrimaryBlogSlug } from "@/lib/server-api";
import { isSolo } from "@/lib/site";

export async function generateMetadata({ params }: { params: Promise<PageParams> }, parent: ResolvingMetadata): Promise<Metadata> {
  return generatePostMetadata({ params }, parent);
}

/** In solo il blog primario ha i post su `/post/{postSlug}`: il vecchio URL `/{primary}/{postSlug}`
 * (anche dai link generati da commenti, newsletter, note...) viene reindirizzato in modo permanente. */
export default async function PublicPostPage({ params }: { params: Promise<PageParams> }) {
  const { blogSlug, postSlug } = await params;
  if (isSolo() && blogSlug === (await getPrimaryBlogSlug())) {
    permanentRedirect(`/post/${postSlug}`);
  }
  return <PublicPostView params={Promise.resolve({ blogSlug, postSlug })} />;
}
