import { getPublicBlog, getPublicBlogPosts, getPrimaryBlogSlug } from "@/lib/server-api";
import { postToFeedItem, type FeedItem, type FeedMeta } from "@/lib/feed";
import { siteUrl } from "@/lib/site";

/** Feed del blog primario per i percorsi di piattaforma `/feed.xml` e `/atom.xml` in solo: stesso contenuto della
 * home `/`. `null` se non è solo, non c'è un blog primario o il blog non è visibile; la route ricade sul feed di
 * piattaforma. */
export async function getPrimaryBlogFeed(
  feedPath: "/feed.xml" | "/atom.xml",
): Promise<{ meta: FeedMeta; items: FeedItem[] } | null> {
  const slug = await getPrimaryBlogSlug();
  if (!slug) return null;
  const [blog, posts] = await Promise.all([getPublicBlog(slug), getPublicBlogPosts(slug).catch(() => null)]);
  if (!blog || !posts) return null;
  return {
    meta: {
      title: blog.title,
      description: blog.description ?? blog.subtitle ?? `Post pubblicati su ${blog.title}.`,
      link: siteUrl(),
      feedUrl: `${siteUrl()}${feedPath}`,
      language: blog.default_locale,
    },
    items: posts.map((post) => postToFeedItem(post)),
  };
}
