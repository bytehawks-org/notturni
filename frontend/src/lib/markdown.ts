import "server-only";

import DOMPurify from "isomorphic-dompurify";
import { JSDOM } from "jsdom";
import MarkdownIt from "markdown-it";

import type { PostNote } from "./types";

// Stessa risoluzione di server-api.ts::BACKEND_INTERNAL_URL — endpoint
// interno alla rete di compose, diverso dall'URL pubblico che risolve solo
// il browser (vedi CLAUDE.md).
const BACKEND_INTERNAL_URL =
  process.env.NOCT_BACKEND_INTERNAL_URL || process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";

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

interface LinkPreviewData {
  title: string | null;
  description: string | null;
  image: string | null;
}

/** Un link salvato come card dall'editor (CLAUDE.md #1) viene inserito come
 * `[url](url "card")`: il title "card" è la stessa convenzione di
 * "sensitive" sulle immagini. Qui recuperiamo l'anteprima (sempre dal vivo,
 * mai una copia salvata: vedi frontend/src/components/editor/LinkPreviewCard.tsx)
 * e sostituiamo il link semplice con la card. Se il fetch fallisce (sito
 * irraggiungibile, timeout) resta un link semplice, mai un errore di
 * rendering della pagina. */
async function resolveLinkCards(html: string): Promise<string> {
  const dom = new JSDOM(`<body>${html}</body>`);
  const { document } = dom.window;
  const cardLinks = Array.from(document.querySelectorAll('a[title="card"]'));
  if (cardLinks.length === 0) return html;

  await Promise.all(
    cardLinks.map(async (a) => {
      const href = a.getAttribute("href");
      if (!href) return;

      let preview: LinkPreviewData | null = null;
      try {
        const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/link-preview?url=${encodeURIComponent(href)}`, {
          cache: "no-store",
        });
        if (res.ok) preview = (await res.json()) as LinkPreviewData;
      } catch {
        // rete non disponibile o timeout: la card degrada a link semplice sotto.
      }

      let hostname = href;
      try {
        hostname = new URL(href).hostname;
      } catch {
        // href relativo/non valido: mostriamo il testo così com'è.
      }

      const card = document.createElement("a");
      card.className = "link-preview-card";
      card.setAttribute("href", href);
      card.setAttribute("target", "_blank");
      card.setAttribute("rel", "noopener noreferrer nofollow");

      if (preview?.image) {
        const img = document.createElement("img");
        img.setAttribute("src", preview.image);
        img.setAttribute("alt", "");
        card.append(img);
      }
      const body = document.createElement("span");
      body.className = "link-preview-card-body";
      const host = document.createElement("span");
      host.className = "link-preview-card-host";
      host.textContent = hostname;
      const title = document.createElement("span");
      title.className = "link-preview-card-title";
      title.textContent = preview?.title || href;
      body.append(host, title);
      if (preview?.description) {
        const description = document.createElement("span");
        description.className = "link-preview-card-description";
        description.textContent = preview.description;
        body.append(description);
      }
      card.append(body);
      a.replaceWith(card);
    })
  );

  return document.body.innerHTML;
}

// Stessa sintassi di app/domain/mentions.py (backend): `@` non preceduto da
// carattere di parola/`@`, seguito da uno username valido (minuscole/cifre con
// `-`/`_` interni). Il gruppo 1 è l'eventuale carattere che precede la `@`.
const MENTION_RE = /(^|[^\w@])@([a-z0-9]+(?:[-_][a-z0-9]+)*)/g;

/** todo/USERS.md #1, todo/EDITOR.md: trasforma le @menzioni nel testo in link
 * al profilo pubblico dell'utente citato. Opera solo sui nodi di testo,
 * saltando quelli già dentro un link, `code` o `pre`. */
function linkifyMentions(html: string): string {
  const dom = new JSDOM(`<body>${html}</body>`);
  const { document } = dom.window;
  const NodeFilter = dom.window.NodeFilter;
  const skip = new Set(["A", "CODE", "PRE"]);

  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  const textNodes: Text[] = [];
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    const parent = (n as Text).parentElement;
    let el: Element | null = parent;
    let skipped = false;
    while (el) {
      if (skip.has(el.tagName)) {
        skipped = true;
        break;
      }
      el = el.parentElement;
    }
    MENTION_RE.lastIndex = 0;
    if (!skipped && MENTION_RE.test((n as Text).data)) textNodes.push(n as Text);
  }

  for (const node of textNodes) {
    const frag = document.createDocumentFragment();
    const text = node.data;
    let lastIndex = 0;
    MENTION_RE.lastIndex = 0;
    let match: RegExpExecArray | null;
    while ((match = MENTION_RE.exec(text)) !== null) {
      const [, lead, username] = match;
      const start = match.index + lead.length;
      if (start > lastIndex) frag.append(document.createTextNode(text.slice(lastIndex, start)));
      const a = document.createElement("a");
      a.setAttribute("href", `/u/${username}`);
      a.className = "mention";
      a.textContent = `@${username}`;
      frag.append(a);
      lastIndex = MENTION_RE.lastIndex;
    }
    if (lastIndex < text.length) frag.append(document.createTextNode(text.slice(lastIndex)));
    node.replaceWith(frag);
  }

  return document.body.innerHTML;
}

/** Rende il Markdown *inline* di una singola nota (nessun wrapper di blocco),
 * sanificato. Usato per l'elenco a piè di pagina e per la bibliografia. */
export function renderNoteInline(markdown: string): string {
  return DOMPurify.sanitize(renderer.renderInline(markdown.trim()));
}

function plainText(html: string): string {
  return DOMPurify.sanitize(html, { ALLOWED_TAGS: [] }).replace(/\s+/g, " ").trim();
}

// Marcatore di nota nel corpo: il link `[n](#nota-n)` prodotto dall'editor
// (sopravvive al round-trip del serializzatore), oppure la forma testuale
// `[^n]` di chi scrive via API.
const BARE_NOTE_REF_RE = /\[\^(\d{1,3})\]/g;

/** todo/EDITOR.md: trasforma i marcatori di nota nel testo in riferimenti in
 * apice (con il testo della nota come tooltip) e accoda l'elenco numerato a
 * piè di pagina. La sorgente è l'elenco strutturato `notes`, non il corpo. */
function renderFootnotes(html: string, notes: PostNote[]): string {
  if (notes.length === 0) return html;
  const dom = new JSDOM(`<body>${html}</body>`);
  const { document } = dom.window;
  const NodeFilter = dom.window.NodeFilter;

  const byIdx = new Map(notes.map((n) => [n.idx, n]));
  const titleOf = (idx: number) => plainText(renderNoteInline(byIdx.get(idx)?.content ?? ""));

  const makeRef = (idx: number): HTMLElement => {
    const sup = document.createElement("sup");
    const known = byIdx.has(idx);
    sup.className = known ? "footnote-ref" : "footnote-ref footnote-ref--missing";
    if (known) {
      sup.id = `fnref-${idx}`;
      sup.setAttribute("title", titleOf(idx));
      const a = document.createElement("a");
      a.setAttribute("href", `#fn-${idx}`);
      a.textContent = String(idx);
      sup.append(a);
    } else {
      sup.textContent = String(idx);
    }
    return sup;
  };

  // 1) link-marcatori [n](#nota-n)
  document.querySelectorAll("a[href]").forEach((a) => {
    const href = a.getAttribute("href") ?? "";
    const m = /^#nota-(\d{1,3})$/.exec(href);
    if (m) a.replaceWith(makeRef(Number(m[1])));
  });

  // 2) marcatori testuali [^n] (fuori da code/pre/link)
  const skip = new Set(["A", "CODE", "PRE"]);
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  const textNodes: Text[] = [];
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    let el: Element | null = (n as Text).parentElement;
    let skipped = false;
    while (el) {
      if (skip.has(el.tagName)) { skipped = true; break; }
      el = el.parentElement;
    }
    BARE_NOTE_REF_RE.lastIndex = 0;
    if (!skipped && BARE_NOTE_REF_RE.test((n as Text).data)) textNodes.push(n as Text);
  }
  for (const node of textNodes) {
    const frag = document.createDocumentFragment();
    const text = node.data;
    let last = 0;
    BARE_NOTE_REF_RE.lastIndex = 0;
    let match: RegExpExecArray | null;
    while ((match = BARE_NOTE_REF_RE.exec(text)) !== null) {
      if (match.index > last) frag.append(document.createTextNode(text.slice(last, match.index)));
      frag.append(makeRef(Number(match[1])));
      last = BARE_NOTE_REF_RE.lastIndex;
    }
    if (last < text.length) frag.append(document.createTextNode(text.slice(last)));
    node.replaceWith(frag);
  }

  // 3) elenco a piè di pagina
  const section = document.createElement("section");
  section.className = "footnotes";
  const heading = document.createElement("h2");
  heading.className = "footnotes-title";
  heading.textContent = "Note";
  const ol = document.createElement("ol");
  for (const note of [...notes].sort((a, b) => a.idx - b.idx)) {
    const li = document.createElement("li");
    li.id = `fn-${note.idx}`;
    li.innerHTML = `${renderNoteInline(note.content)} <a class="footnote-backref" href="#fnref-${note.idx}" aria-label="Torna al testo">↩</a>`;
    ol.append(li);
  }
  section.append(heading, ol);
  document.body.append(section);

  return document.body.innerHTML;
}

export interface RenderOptions {
  /** Se true, le @menzioni diventano link al profilo (todo/EDITOR.md: il
   * proprietario del blog può disattivarle). Default: true. */
  mentions?: boolean;
  /** Note a piè di pagina del post (todo/EDITOR.md). Se presenti, i marcatori
   * nel testo diventano riferimenti in apice e viene accodato l'elenco. */
  notes?: PostNote[];
}

export async function renderMarkdown(markdown: string, options: RenderOptions = {}): Promise<string> {
  const rawHtml = renderer.render(markdown);
  const cleanHtml = DOMPurify.sanitize(rawHtml);
  const withImages = wrapSensitiveImages(cleanHtml);
  const withCards = await resolveLinkCards(withImages);
  const withMentions = options.mentions === false ? withCards : linkifyMentions(withCards);
  return options.notes && options.notes.length > 0
    ? renderFootnotes(withMentions, options.notes)
    : withMentions;
}

/** Estratto in solo testo per anteprime (card del feed, meta description):
 * rende a HTML, sanifica, poi butta via anche i tag rimasti. I marcatori di
 * nota (`[n](#nota-n)` e `[^n]`) vengono tolti per non sporcare l'anteprima. */
export function excerpt(markdown: string, maxLength = 160): string {
  const stripped = markdown
    .replace(/\[(\d{1,3})\]\(#nota-\d{1,3}\)/g, "")
    .replace(BARE_NOTE_REF_RE, "");
  const text = DOMPurify.sanitize(renderer.render(stripped), { ALLOWED_TAGS: [] })
    .replace(/\s+/g, " ")
    .trim();
  if (text.length <= maxLength) return text;
  return `${text.slice(0, maxLength).trimEnd()}…`;
}
