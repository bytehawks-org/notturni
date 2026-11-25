import type { Metadata, ResolvingMetadata } from "next";
import { notFound } from "next/navigation";

import { PublicPostView, generatePostMetadata } from "@/components/blog/PublicPostView";
import { getPrimaryBlogSlug } from "@/lib/server-api";
import { isSolo } from "@/lib/site";

interface PageParams {
  slug: string;
}

/** Post del blog primario di un'istanza solo (`/post/{slug}`). Su piattaforma non esiste: 404. */
async function resolvePrimary(slug: string) {
  if (!isSolo()) notFound();
  const primary = await getPrimaryBlogSlug();
  if (!primary) notFound();
  return { blogSlug: primary, postSlug: slug };
}

export async function generateMetadata({ params }: { params: Promise<PageParams> }, parent: ResolvingMetadata): Promise<Metadata> {
  const { slug } = await params;
  const resolved = await resolvePrimary(slug);
  return generatePostMetadata({ params: Promise.resolve(resolved) }, parent);
}

export default async function PostBySlugPage({ params }: { params: Promise<PageParams> }) {
  const { slug } = await params;
  const resolved = await resolvePrimary(slug);
  return <PublicPostView params={Promise.resolve(resolved)} />;
}
