import "server-only";

import { blogLinks, type BlogLinks } from "@/lib/blog-path";
import { getPrimaryBlogSlug } from "@/lib/server-api";

/** `blogLinks` con il blog primario letto dal backend (`getPrimaryBlogSlug`). Usare nelle pagine del blog: senza il
 * primario `homeHref` del blog primario in solo non sarebbe `/`. */
export async function currentBlogLinks(blogSlug: string): Promise<BlogLinks> {
  return blogLinks(blogSlug, await getPrimaryBlogSlug());
}
