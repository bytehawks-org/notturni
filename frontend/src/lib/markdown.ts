import "server-only";

import DOMPurify from "isomorphic-dompurify";
import { JSDOM } from "jsdom";
import MarkdownIt from "markdown-it";

import { REVALIDATE_SECONDS } from "./revalidate";
import type { PostNote } from "./types";

// Stessa risoluzione di server-api.ts::BACKEND_INTERNAL_URL — endpoint
// interno alla rete di compose, diverso dall'URL pubblico che risolve solo
// il browser (vedi CLAUDE.md).
const BACKEND_INTERNAL_URL =
  process.env.NOCT_BACKEND_INTERNAL_URL || process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000";

// La fetch dell'anteprima di un link è verso un sito esterno arbitrario: un
// timeout esplicito evita che un host lento tenga bloccato tutto il rendering
// della pagina del post. Allo scadere, la card degrada (solo hostname/URL).
const LINK_PREVIEW_TIMEOUT_MS = 2000;

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
 * (checkbox nascosto + selettore ~), niente JavaScript lato client.
 *
 * Aggiunge anche il pulsante di ingrandimento (Rifinitura #1,
 * components/Lightbox.tsx): compare solo dopo la rivelazione (stesso trucco
 * CSS, `.sensitive-image-toggle:checked ~ .lightbox-expand-btn`), mai sullo
 * stesso click che rivela l'immagine — la Lightbox stessa (client-side)
 * intercetta il click su questo pulsante via delega globale, non serve
 * altro JS qui. Le immagini *non* sensibili sono invece cliccabili subito,
 * marcate `data-lightbox` in `renderPipeline`.
 *
 * Muta `document` in place: fa parte della pipeline di `renderMarkdown`, che
 * fa un solo parse DOM per tutte le trasformazioni. */
function wrapSensitiveImages(document: Document): void {
  document.querySelectorAll('img[title^="sensitive"]').forEach((img) => {
    const wrapper = document.createElement("label");
    wrapper.className = "sensitive-image-wrapper";
    const toggle = document.createElement("input");
    toggle.type = "checkbox";
    toggle.className = "sensitive-image-toggle";
    const overlay = document.createElement("span");
    overlay.className = "sensitive-image-overlay";
    overlay.textContent = "Contenuto sensibile — clicca per vedere";
    const src = img.getAttribute("src") ?? "";
    const expandBtn = document.createElement("button");
    expandBtn.type = "button";
    expandBtn.className = "lightbox-expand-btn";
    expandBtn.setAttribute("data-lightbox-src", src);
    expandBtn.setAttribute("data-lightbox-alt", img.getAttribute("alt") ?? "");
    expandBtn.setAttribute("aria-label", "Ingrandisci");
    expandBtn.innerHTML =
      '<svg viewBox="0 0 18 18" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M11 3.5h3.5V7"/><path d="M14.5 3.5 10 8"/><path d="M7 14.5H3.5V11"/><path d="M3.5 14.5 8 10"/></svg>';
    img.replaceWith(wrapper);
    wrapper.append(toggle, img, overlay, expandBtn);
  });
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
 * irraggiungibile, timeout) la card degrada a solo hostname/URL, mai un
 * errore di rendering della pagina.
 *
 * Muta `document` in place (vedi `wrapSensitiveImages`). */
async function resolveLinkCards(document: Document): Promise<void> {
  const cardLinks = Array.from(document.querySelectorAll('a[title="card"]'));
  if (cardLinks.length === 0) return;

  await Promise.all(
    cardLinks.map(async (a) => {
      const href = a.getAttribute("href");
      if (!href) return;

      let preview: LinkPreviewData | null = null;
      try {
        // Il backend ha una propria cache (Redis + tabella dedicata,
        // condivisa/deduplicata per URL — app/domain/link_preview.py): non
        // serve più `no-store` qui, la stessa finestra a tempo delle altre
        // fetch pubbliche basta, e la pagina del post con una card di link
        // può tornare cacheabile invece di restare sempre dinamica.
        const res = await fetch(`${BACKEND_INTERNAL_URL}/api/v1/link-preview?url=${encodeURIComponent(href)}`, {
          next: { revalidate: REVALIDATE_SECONDS },
          signal: AbortSignal.timeout(LINK_PREVIEW_TIMEOUT_MS),
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
}

// Stessa sintassi di app/domain/mentions.py (backend): `@` non preceduto da
// carattere di parola/`@`, seguito da uno username valido (minuscole/cifre con
// `-`/`_` interni). Il gruppo 1 è l'eventuale carattere che precede la `@`.
const MENTION_RE = /(^|[^\w@])@([a-z0-9]+(?:[-_][a-z0-9]+)*)/g;

/** todo/USERS.md #1, todo/EDITOR.md: trasforma le @menzioni nel testo in link
 * al profilo pubblico dell'utente citato. Opera solo sui nodi di testo,
 * saltando quelli già dentro un link, `code` o `pre`.
 *
 * Muta `document` in place (vedi `wrapSensitiveImages`). */
function linkifyMentions(document: Document): void {
  const view = document.defaultView;
  if (!view) return;
  const NodeFilter = view.NodeFilter;
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
}

/** Rende il Markdown *inline* di una singola nota (nessun wrapper di blocco),
 * sanificato. Usato per l'elenco a piè di pagina e per la bibliografia. */
export function renderNoteInline(markdown: string): string {
  return DOMPurify.sanitize(renderer.renderInline(markdown.trim()));
}

/** Rende un blocco di Markdown libero (immagini, link, paragrafi) senza la
 * pipeline completa di `renderMarkdown`: niente parse JSDOM aggiuntivo, niente
 * card di anteprima dei link (fetch di rete verso siti esterni) né avvolgimento
 * delle immagini sensibili — non ha senso per un footer mostrato su ogni
 * pagina della piattaforma e di ogni blog. Solo render + sanificazione. */
export function renderSimpleMarkdown(markdown: string): string {
  return DOMPurify.sanitize(renderer.render(markdown.trim()));
}

function plainText(html: string): string {
  return DOMPurify.sanitize(html, { ALLOWED_TAGS: [] }).replace(/\s+/g, " ").trim();
}

// Marcatore di nota nel corpo: il link `[n](#nota-n)` prodotto dall'editor
// (sopravvive al round-trip del serializzatore), oppure la forma testuale
// `[^n]` di chi scrive via API.
const BARE_NOTE_REF_RE = /\[\^(\d{1,3})\]/g;

/** Riga di citazione coi campi bibliografici opzionali di una nota (modal
 * "Nota" nell'editor), sotto il testo libero — `null` se la nota non ne ha
 * nessuno. Costruita con nodi DOM (mai concatenazione di HTML grezzo): a
 * differenza di `note.content`, questi campi non passano da
 * `renderNoteInline`/DOMPurify, sono testo semplice inserito come
 * `textContent`. */
function buildNoteCitationElement(
  document: Document,
  note: { author?: string | null; title?: string | null; page?: string | null; isbn?: string | null; doi?: string | null }
): HTMLElement | null {
  if (!note.author && !note.title && !note.page && !note.isbn && !note.doi) return null;

  const p = document.createElement("p");
  p.className = "footnote-citation";
  const parts: (string | HTMLElement)[] = [];
  if (note.author) parts.push(note.author);
  if (note.title) {
    const em = document.createElement("em");
    em.textContent = note.title;
    parts.push(em);
  }
  if (note.page) parts.push(`p. ${note.page}`);
  if (note.isbn) parts.push(`ISBN ${note.isbn}`);

  parts.forEach((part, i) => {
    if (i > 0) p.append(" · ");
    p.append(part);
  });

  if (note.doi) {
    if (parts.length > 0) p.append(" · ");
    const a = document.createElement("a");
    a.setAttribute("href", `https://doi.org/${note.doi}`);
    a.setAttribute("target", "_blank");
    a.setAttribute("rel", "noopener noreferrer nofollow");
    a.textContent = `doi.org/${note.doi}`;
    p.append(a);
  }

  return p;
}

/** todo/EDITOR.md: trasforma i marcatori di nota nel testo in riferimenti in
 * apice (con il testo della nota come tooltip) e accoda l'elenco numerato a
 * piè di pagina. La sorgente è l'elenco strutturato `notes`, non il corpo.
 *
 * Muta `document` in place (vedi `wrapSensitiveImages`). */
function renderFootnotes(document: Document, notes: PostNote[], labels: FootnoteLabels): void {
  if (notes.length === 0) return;
  const view = document.defaultView;
  if (!view) return;
  const NodeFilter = view.NodeFilter;

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
  heading.textContent = labels.title;
  const ol = document.createElement("ol");
  for (const note of [...notes].sort((a, b) => a.idx - b.idx)) {
    const li = document.createElement("li");
    li.id = `fn-${note.idx}`;
    li.innerHTML = `${renderNoteInline(note.content)} <a class="footnote-backref" href="#fnref-${note.idx}" aria-label="${labels.backToText}">↩</a>`;
    const citation = buildNoteCitationElement(document, note);
    if (citation) li.append(citation);
    ol.append(li);
  }
  section.append(heading, ol);
  document.body.append(section);
}

export interface FootnoteLabels {
  title: string;
  backToText: string;
}

const DEFAULT_FOOTNOTE_LABELS: FootnoteLabels = { title: "Note", backToText: "Torna al testo" };

export interface PostHeading {
  id: string;
  text: string;
  level: 2 | 3;
}

const slugifyHeading = (text: string): string =>
  text
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 60) || "sezione";

/** Mockup 1a ("In this post"): assegna un id stabile a h2/h3 e ne restituisce
 * l'elenco per l'indice laterale. Muta `document` in place. */
function anchorHeadings(document: Document): PostHeading[] {
  const seen = new Map<string, number>();
  const headings: PostHeading[] = [];
  document.querySelectorAll("h2, h3").forEach((el) => {
    if (el.closest(".footnotes")) return;
    const text = (el.textContent ?? "").replace(/\s+/g, " ").trim();
    if (!text) return;
    const base = slugifyHeading(text);
    const n = (seen.get(base) ?? 0) + 1;
    seen.set(base, n);
    const id = n === 1 ? base : `${base}-${n}`;
    el.id = id;
    headings.push({ id, text, level: el.tagName === "H2" ? 2 : 3 });
  });
  return headings;
}

export interface RenderOptions {
  /** Se true, le @menzioni diventano link al profilo (todo/EDITOR.md: il
   * proprietario del blog può disattivarle). Default: true. */
  mentions?: boolean;
  /** Note a piè di pagina del post (todo/EDITOR.md). Se presenti, i marcatori
   * nel testo diventano riferimenti in apice e viene accodato l'elenco. */
  notes?: PostNote[];
  /** Etichette dell'elenco note nella lingua dell'interfaccia (next-intl). */
  footnoteLabels?: FootnoteLabels;
}

export interface RenderedPost {
  html: string;
  headings: PostHeading[];
}

/** Come `renderMarkdown`, ma restituisce anche l'indice dei titoli (con id
 * ancorabili) per la colonna "In questo post" della pagina pubblica. */
export async function renderPost(markdown: string, options: RenderOptions = {}): Promise<RenderedPost> {
  return renderPipeline(markdown, options, true);
}

export async function renderMarkdown(markdown: string, options: RenderOptions = {}): Promise<string> {
  return (await renderPipeline(markdown, options, false)).html;
}

async function renderPipeline(markdown: string, options: RenderOptions, withHeadings: boolean): Promise<RenderedPost> {
  const rawHtml = renderer.render(markdown);
  const cleanHtml = DOMPurify.sanitize(rawHtml);

  // Un solo parse DOM per l'intera pipeline: prima ogni passo faceva
  // `new JSDOM(...)` e ri-serializzava (4 parse per render, ripetuti a ogni
  // pagina). Ora le trasformazioni mutano lo stesso `document` in place.
  const dom = new JSDOM(`<body>${cleanHtml}</body>`);
  const { document } = dom.window;

  wrapSensitiveImages(document);
  // Immagini di contenuto non segnalate come sensibili: cliccabili subito
  // per la Lightbox (Rifinitura #1) — quelle sensibili restano escluse (sono
  // comunque ancora <img> dentro il wrapper appena creato sopra, non
  // rimosse dal documento): hanno il proprio pulsante dedicato, aggiunto da
  // wrapSensitiveImages, mai la stessa immagine cliccabile direttamente
  // (altrimenti il primo click aprirebbe subito la lightbox invece di
  // limitarsi a rivelarla).
  document.querySelectorAll("img").forEach((img) => {
    if (!img.closest(".sensitive-image-wrapper")) img.setAttribute("data-lightbox", "1");
  });
  await resolveLinkCards(document);
  if (options.mentions !== false) linkifyMentions(document);
  const headings = withHeadings ? anchorHeadings(document) : [];
  if (options.notes && options.notes.length > 0) {
    renderFootnotes(document, options.notes, options.footnoteLabels ?? DEFAULT_FOOTNOTE_LABELS);
  }

  return { html: document.body.innerHTML, headings };
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
