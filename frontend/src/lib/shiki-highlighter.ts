import "server-only";

import { createHighlighter, type Highlighter } from "shiki";

/** Temi chiaro/scuro dei blocchi di codice: doppio tema Shiki con variabili
 * CSS (`--shiki-light`/`--shiki-dark`), letto da globals.css in base
 * all'attributo `data-theme` sulla root (stesso meccanismo del resto del
 * tema chiaro/scuro della piattaforma). */
export const SHIKI_THEME_LIGHT = "github-light";
export const SHIKI_THEME_DARK = "github-dark";

// Un solo highlighter condiviso per il processo del server Next.js (creato
// senza linguaggi: caricati su richiesta da ensureLanguagesLoaded, evita di
// pagare il costo di ~200 grammatiche Shiki bundlate se un'installazione non
// le userà mai tutte).
let highlighterPromise: Promise<Highlighter> | null = null;

function getHighlighter(): Promise<Highlighter> {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighter({ themes: [SHIKI_THEME_LIGHT, SHIKI_THEME_DARK], langs: [] });
  }
  return highlighterPromise;
}

const FENCE_LANG_RE = /^`{3,}[ \t]*([a-zA-Z0-9_+#.-]+)/gm;
const NO_HIGHLIGHT_LANGS = new Set(["text", "plaintext", "plain", "txt", ""]);

/** Carica (se non già caricati sull'highlighter condiviso) i linguaggi
 * citati nei fence ```` ```lang ```` di un documento, **prima** di un render
 * sincrono di markdown-it: il caricamento delle grammatiche Shiki è
 * asincrono, incompatibile con l'hook `highlight` sincrono di markdown-it —
 * questa funzione va sempre attesa prima di `renderer.render()`. Un
 * linguaggio sconosciuto a Shiki non blocca il rendering del post: il fence
 * resta testo semplice senza evidenziazione (fallito silenziosamente qui,
 * gestito anche lato `highlight` callback). */
export async function ensureLanguagesLoaded(markdown: string): Promise<Highlighter> {
  const highlighter = await getHighlighter();
  const wanted = new Set<string>();
  let match: RegExpExecArray | null;
  FENCE_LANG_RE.lastIndex = 0;
  while ((match = FENCE_LANG_RE.exec(markdown))) {
    const lang = match[1].toLowerCase();
    if (!NO_HIGHLIGHT_LANGS.has(lang)) wanted.add(lang);
  }
  const loaded = new Set(highlighter.getLoadedLanguages());
  const toLoad = [...wanted].filter((lang) => !loaded.has(lang));
  await Promise.all(toLoad.map((lang) => highlighter.loadLanguage(lang as never).catch(() => undefined)));
  return highlighter;
}

/** Callback `highlight` di markdown-it (sincrona): l'highlighter passato è
 * già stato preparato da `ensureLanguagesLoaded` per il documento corrente.
 * Ritorna `""` (nessuna evidenziazione, markdown-it ricade sull'escape
 * semplice) se il linguaggio non è riconosciuto o non è stato caricato. */
export function highlightCode(highlighter: Highlighter, code: string, lang: string): string {
  const language = lang.toLowerCase();
  if (NO_HIGHLIGHT_LANGS.has(language) || !highlighter.getLoadedLanguages().includes(language)) return "";
  try {
    return highlighter.codeToHtml(code, {
      lang: language,
      themes: { light: SHIKI_THEME_LIGHT, dark: SHIKI_THEME_DARK },
      defaultColor: false,
    });
  } catch {
    return "";
  }
}
