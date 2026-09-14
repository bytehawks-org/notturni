import "server-only";

import { excerpt } from "./markdown";
import { SITE_URL } from "./site";
import type { Post } from "./types";

/** RSS 2.0 / Atom 1.0 (richiesta esplicita): feed di piattaforma
 * (`app/feed.xml`, `app/atom.xml`) e per blog (`app/[blogSlug]/feed.xml`,
 * `.../atom.xml`) — vedi le route handler in `app/`. Solo escape/formato
 * XML qui: i dati arrivano già filtrati per visibilità pubblica dagli stessi
 * endpoint usati dal resto del sito (`getPublicFeed`/`getPublicBlogPosts`).
 * Solo titolo + estratto per ogni voce, non il contenuto completo: un post
 * può avere immagini segnalate sensibili (sfocate via CSS sulla pagina, un
 * trucco che un lettore di feed non applica) — l'estratto è testo puro,
 * nessun rischio di mostrarle senza avviso. */

const FEED_ITEM_LIMIT_CHARS = 400;

function escapeXml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;");
}

export interface FeedMeta {
  title: string;
  description: string;
  /** URL assoluta della pagina HTML che questo feed rappresenta. */
  link: string;
  /** URL assoluta del feed stesso (self-link, richiesto da molti validator). */
  feedUrl: string;
  language: string;
}

export interface FeedItem {
  title: string;
  link: string;
  description: string;
  guid: string;
  pubDate: string | null;
  author: string | null;
}

/** Un post → voce di feed: solo titolo/estratto, mai il Markdown completo —
 * vedi la nota in cima al file sul perché. */
export function postToFeedItem(post: Post): FeedItem {
  return {
    title: post.title,
    link: `${SITE_URL}${post.permalink}`,
    description: excerpt(post.content, FEED_ITEM_LIMIT_CHARS),
    guid: `${SITE_URL}${post.permalink}`,
    pubDate: post.published_at,
    author: post.author_display_name,
  };
}

export function buildRss(meta: FeedMeta, items: FeedItem[]): string {
  const itemsXml = items
    .map(
      (item) => `
    <item>
      <title>${escapeXml(item.title)}</title>
      <link>${escapeXml(item.link)}</link>
      <guid isPermaLink="true">${escapeXml(item.guid)}</guid>
      ${item.pubDate ? `<pubDate>${new Date(item.pubDate).toUTCString()}</pubDate>` : ""}
      ${item.author ? `<dc:creator>${escapeXml(item.author)}</dc:creator>` : ""}
      <description>${escapeXml(item.description)}</description>
    </item>`
    )
    .join("");

  return `<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom" xmlns:dc="http://purl.org/dc/elements/1.1/">
  <channel>
    <title>${escapeXml(meta.title)}</title>
    <link>${escapeXml(meta.link)}</link>
    <description>${escapeXml(meta.description)}</description>
    <language>${escapeXml(meta.language)}</language>
    <atom:link href="${escapeXml(meta.feedUrl)}" rel="self" type="application/rss+xml" />${itemsXml}
  </channel>
</rss>
`;
}

export function buildAtom(meta: FeedMeta, items: FeedItem[]): string {
  const mostRecent = items.find((i) => i.pubDate)?.pubDate;
  const updated = mostRecent ? new Date(mostRecent).toISOString() : new Date().toISOString();

  const entriesXml = items
    .map((item) => {
      const entryUpdated = item.pubDate ? new Date(item.pubDate).toISOString() : updated;
      return `
  <entry>
    <title>${escapeXml(item.title)}</title>
    <link href="${escapeXml(item.link)}" />
    <id>${escapeXml(item.guid)}</id>
    <updated>${entryUpdated}</updated>
    ${item.author ? `<author><name>${escapeXml(item.author)}</name></author>` : ""}
    <summary>${escapeXml(item.description)}</summary>
  </entry>`;
    })
    .join("");

  return `<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>${escapeXml(meta.title)}</title>
  <link href="${escapeXml(meta.link)}" />
  <link href="${escapeXml(meta.feedUrl)}" rel="self" />
  <id>${escapeXml(meta.link)}</id>
  <updated>${updated}</updated>${entriesXml}
</feed>
`;
}

export const FEED_HEADERS = {
  rss: { "Content-Type": "application/rss+xml; charset=utf-8" },
  atom: { "Content-Type": "application/atom+xml; charset=utf-8" },
} as const;
