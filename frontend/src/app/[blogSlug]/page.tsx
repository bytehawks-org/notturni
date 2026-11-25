import type { Metadata, ResolvingMetadata } from "next";
import { permanentRedirect } from "next/navigation";

import { BlogHome, blogHomeMetadata } from "@/components/blog/BlogHome";
import { isOnBlogSubdomain } from "@/lib/blog-path";
import { getPrimaryBlogSlug } from "@/lib/server-api";
import { isPrimaryBlog } from "@/lib/site";

interface PageParams {
  blogSlug: string;
}

export async function generateMetadata({ params }: { params: Promise<PageParams> }, parent: ResolvingMetadata): Promise<Metadata> {
  const { blogSlug } = await params;
  return blogHomeMetadata({ blogSlug }, parent);
}

/** Home del blog a `/{blogSlug}`. In solo la home del blog primario sta su `/`: qui il path esatto
 * `/{primarySlug}` reindirizza a `/` (le sottopagine restano su `/{primarySlug}/...`). Sul sottodominio il proxy
 * riscrive `/` in `/{slug}`: lì non si reindirizza, altrimenti il loop. */
export default async function BlogHomePage({
  params,
  searchParams,
}: {
  params: Promise<PageParams>;
  searchParams: Promise<{ category?: string }>;
}) {
  const [{ blogSlug }, { category }] = await Promise.all([params, searchParams]);
  if (isPrimaryBlog(blogSlug, await getPrimaryBlogSlug()) && !(await isOnBlogSubdomain(blogSlug))) {
    permanentRedirect(category ? `/?category=${encodeURIComponent(category)}` : "/");
  }
  return <BlogHome blogSlug={blogSlug} category={category} />;
}
