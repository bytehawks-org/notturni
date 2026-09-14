import { notFound } from "next/navigation";

import { FEED_HEADERS, buildAtom, postToFeedItem } from "@/lib/feed";
import { getPublicBlog, getPublicBlogPosts } from "@/lib/server-api";
import { SITE_URL } from "@/lib/site";

export const dynamic = "force-dynamic";

interface RouteParams {
  blogSlug: string;
}

/** Atom 1.0 del blog — stessi dati di `feed.xml` (RSS 2.0) per questo blog. */
export async function GET(_request: Request, { params }: { params: Promise<RouteParams> }) {
  const { blogSlug } = await params;
  const [blog, posts] = await Promise.all([
    getPublicBlog(blogSlug),
    getPublicBlogPosts(blogSlug).catch(() => null),
  ]);
  if (!blog || !posts) notFound();

  const xml = buildAtom(
    {
      title: blog.title,
      description: blog.description ?? blog.subtitle ?? `Post pubblicati su ${blog.title}.`,
      link: `${SITE_URL}/${blogSlug}`,
      feedUrl: `${SITE_URL}/${blogSlug}/atom.xml`,
      language: blog.default_locale,
    },
    posts.map((post) => postToFeedItem(post))
  );
  return new Response(xml, { headers: FEED_HEADERS.atom });
}
