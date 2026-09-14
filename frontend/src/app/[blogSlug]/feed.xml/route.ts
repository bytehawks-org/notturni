import { notFound } from "next/navigation";

import { FEED_HEADERS, buildRss, postToFeedItem } from "@/lib/feed";
import { getPublicBlog, getPublicBlogPosts } from "@/lib/server-api";
import { SITE_URL } from "@/lib/site";

export const dynamic = "force-dynamic";

interface RouteParams {
  blogSlug: string;
}

/** RSS 2.0 del blog (richiesta esplicita) — stessi post pubblicati mostrati
 * sulla sua home pubblica (`GET /blogs/{slug}/posts`). `404` se il blog non
 * esiste o non è pubblicamente visibile, stessa regola della sua home. */
export async function GET(_request: Request, { params }: { params: Promise<RouteParams> }) {
  const { blogSlug } = await params;
  const [blog, posts] = await Promise.all([
    getPublicBlog(blogSlug),
    getPublicBlogPosts(blogSlug).catch(() => null),
  ]);
  if (!blog || !posts) notFound();

  const xml = buildRss(
    {
      title: blog.title,
      description: blog.description ?? blog.subtitle ?? `Post pubblicati su ${blog.title}.`,
      link: `${SITE_URL}/${blogSlug}`,
      feedUrl: `${SITE_URL}/${blogSlug}/feed.xml`,
      language: blog.default_locale,
    },
    posts.map((post) => postToFeedItem(post))
  );
  return new Response(xml, { headers: FEED_HEADERS.rss });
}
