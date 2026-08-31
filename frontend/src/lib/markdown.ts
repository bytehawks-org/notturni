import "server-only";

import DOMPurify from "isomorphic-dompurify";
import MarkdownIt from "markdown-it";

// Il backend salva Markdown grezzo, non fidato (può arrivare anche da
// chiamate dirette all'API, non solo dall'editor WYSIWYG) — vedi API.md:
// "conversione a HTML (con sanificazione) è responsabilità del frontend al
// momento della lettura". `html: false` impedisce già a markdown-it di
// lasciar passare tag HTML scritti a mano nel sorgente; DOMPurify è comunque
// una seconda barriera sull'HTML che markdown-it stesso genera (es. src di
// immagini/link), difesa in profondità più che ridondanza.
const renderer = new MarkdownIt({ html: false, linkify: true, breaks: false });

export function renderMarkdown(markdown: string): string {
  const rawHtml = renderer.render(markdown);
  return DOMPurify.sanitize(rawHtml);
}

/** Estratto in solo testo per anteprime (card del feed, meta description):
 * rende a HTML, sanifica, poi butta via anche i tag rimasti. */
export function excerpt(markdown: string, maxLength = 160): string {
  const text = DOMPurify.sanitize(renderer.render(markdown), { ALLOWED_TAGS: [] })
    .replace(/\s+/g, " ")
    .trim();
  if (text.length <= maxLength) return text;
  return `${text.slice(0, maxLength).trimEnd()}…`;
}
