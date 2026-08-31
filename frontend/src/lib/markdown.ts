import "server-only";

import DOMPurify from "isomorphic-dompurify";
import { JSDOM } from "jsdom";
import MarkdownIt from "markdown-it";

// Il backend salva Markdown grezzo, non fidato (può arrivare anche da
// chiamate dirette all'API, non solo dall'editor WYSIWYG) — vedi API.md:
// "conversione a HTML (con sanificazione) è responsabilità del frontend al
// momento della lettura". `html: false` impedisce già a markdown-it di
// lasciar passare tag HTML scritti a mano nel sorgente; DOMPurify è comunque
// una seconda barriera sull'HTML che markdown-it stesso genera (es. src di
// immagini/link), difesa in profondità più che ridondanza.
const renderer = new MarkdownIt({ html: false, linkify: true, breaks: false });

/** Un'immagine segnalata sensibile dalla moderazione automatica (vedi
 * API.md) viene inserita dall'editor come `![alt](url "sensitive")`: il
 * title "sensitive" è la convenzione con cui il Markdown porta con sé
 * l'informazione, senza bisogno di una tabella dedicata. Qui la trasformiamo
 * in un blocco sfocato, cliccabile per rivelarla — un puro trucco CSS
 * (checkbox nascosto + selettore ~), niente JavaScript lato client. */
function wrapSensitiveImages(html: string): string {
  const dom = new JSDOM(`<body>${html}</body>`);
  const document = dom.window.document;
  document.querySelectorAll('img[title="sensitive"]').forEach((img) => {
    const wrapper = document.createElement("label");
    wrapper.className = "sensitive-image-wrapper";
    const toggle = document.createElement("input");
    toggle.type = "checkbox";
    toggle.className = "sensitive-image-toggle";
    const overlay = document.createElement("span");
    overlay.className = "sensitive-image-overlay";
    overlay.textContent = "Contenuto sensibile — clicca per vedere";
    img.replaceWith(wrapper);
    wrapper.append(toggle, img, overlay);
  });
  return document.body.innerHTML;
}

export function renderMarkdown(markdown: string): string {
  const rawHtml = renderer.render(markdown);
  const cleanHtml = DOMPurify.sanitize(rawHtml);
  return wrapSensitiveImages(cleanHtml);
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
