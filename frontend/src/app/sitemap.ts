import type { MetadataRoute } from "next";

import { getPublicPlatformPages, getSitemapEntries } from "@/lib/server-api";
import { SITE_URL } from "@/lib/site";

// Stesso motivo di robots.ts: senza questo Next prova a pre-renderizzare
// /sitemap.xml in fase di build e fallisce se il backend non è raggiungibile.
export const dynamic = "force-dynamic";

export default async function sitemap(): Promise<MetadataRoute.Sitemap> {
  const [{ blogs, posts }, pages] = await Promise.all([
    getSitemapEntries(),
    getPublicPlatformPages().catch(() => []),
  ]);

  const entries: MetadataRoute.Sitemap = [{ url: SITE_URL, changeFrequency: "daily", priority: 1 }];

  for (const page of pages) {
    entries.push({ url: `${SITE_URL}/p/${page.slug}`, changeFrequency: "monthly", priority: 0.5 });
  }
  for (const blog of blogs) {
    entries.push({
      url: `${SITE_URL}/${blog.slug}`,
      lastModified: blog.updated_at,
      changeFrequency: "daily",
      priority: 0.7,
    });
  }
  for (const post of posts) {
    entries.push({
      url: `${SITE_URL}${post.permalink}`,
      lastModified: post.updated_at,
      changeFrequency: "monthly",
      priority: 0.6,
    });
  }

  return entries;
}
