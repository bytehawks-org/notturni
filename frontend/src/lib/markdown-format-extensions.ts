import type MarkdownItCtor from "markdown-it";

type MarkdownIt = InstanceType<typeof MarkdownItCtor>;

/**
 * Sintassi non-CommonMark, condivisa tra l'editor (`tiptap-markdown`, via
 * `parse.setup` sulle estensioni in components/editor/markdownFormatting.ts)
 * e il renderer pubblico (`src/lib/markdown.ts`): `++testo++` per il
 * sottolineato, `{: .left|.center|.right}` a fine paragrafo/titolo per
 * l'allineamento. Scelte deliberatamente al posto di abilitare `html: true`
 * in markdown-it: il Markdown salvato può arrivare anche da chiamate dirette
 * all'API, non fidate (vedi il commento su `renderer` in markdown.ts) — sia
 * l'editor sia il renderer pubblico restano con `html: false`, queste due
 * sintassi restano gli unici modi per produrre quel markup, con un output
 * fisso e noto (mai testo libero dell'utente).
 *
 * Ogni funzione è idempotente (guardia su `md`): `tiptap-markdown` richiama
 * `parse.setup` ad ogni `parse()`, non solo alla creazione dell'istanza
 * `markdown-it`.
 */

// Stessa firma di rules_inline/strikethrough.ts in markdown-it (marker `~`,
// tag `s`), qui con marker `+` e tag `u` — vedi la spiegazione sopra sul
// perché non si usa semplicemente `html: true` + `<u>` scritto a mano.
export function underlineMarkdownPlugin(md: MarkdownIt): void {
  const flagged = md as unknown as { __notturniUnderlineInstalled?: boolean };
  if (flagged.__notturniUnderlineInstalled) return;
  flagged.__notturniUnderlineInstalled = true;

  const PLUS = 0x2b; // '+'

  // eslint-disable-next-line @typescript-eslint/no-explicit-any -- StateInline non è tipizzato dai .d.ts di markdown-it
  function tokenize(state: any, silent: boolean): boolean {
    const start = state.pos;
    if (silent || state.src.charCodeAt(start) !== PLUS) return false;

    const scanned = state.scanDelims(state.pos, true);
    let len = scanned.length;
    if (len < 2) return false;

    let token;
    if (len % 2) {
      token = state.push("text", "", 0);
      token.content = "+";
      len--;
    }
    for (let i = 0; i < len; i += 2) {
      token = state.push("text", "", 0);
      token.content = "++";
      state.delimiters.push({
        marker: PLUS,
        length: 0,
        token: state.tokens.length - 1,
        end: -1,
        open: scanned.can_open,
        close: scanned.can_close,
      });
    }
    state.pos += scanned.length;
    return true;
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any -- vedi sopra
  function postProcessPairs(state: any, delimiters: any[]): void {
    const loneMarkers: number[] = [];
    for (let i = 0; i < delimiters.length; i++) {
      const startDelim = delimiters[i];
      if (startDelim.marker !== PLUS || startDelim.end === -1) continue;
      const endDelim = delimiters[startDelim.end];

      const openToken = state.tokens[startDelim.token];
      openToken.type = "underline_open";
      openToken.tag = "u";
      openToken.nesting = 1;
      openToken.markup = "++";
      openToken.content = "";

      const closeToken = state.tokens[endDelim.token];
      closeToken.type = "underline_close";
      closeToken.tag = "u";
      closeToken.nesting = -1;
      closeToken.markup = "++";
      closeToken.content = "";

      const beforeClose = state.tokens[endDelim.token - 1];
      if (beforeClose.type === "text" && beforeClose.content === "+") {
        loneMarkers.push(endDelim.token - 1);
      }
    }
    while (loneMarkers.length) {
      const i = loneMarkers.pop() as number;
      let j = i + 1;
      while (j < state.tokens.length && state.tokens[j].type === "underline_close") j++;
      j--;
      if (i !== j) {
        const tmp = state.tokens[j];
        state.tokens[j] = state.tokens[i];
        state.tokens[i] = tmp;
      }
    }
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any -- vedi sopra
  function postProcess(state: any): void {
    postProcessPairs(state, state.delimiters);
    const tokensMeta = state.tokens_meta;
    for (let curr = 0; curr < tokensMeta.length; curr++) {
      const delimiters = tokensMeta[curr]?.delimiters;
      if (delimiters) postProcessPairs(state, delimiters);
    }
  }

  md.inline.ruler.before("emphasis", "underline", tokenize);
  md.inline.ruler2.before("emphasis", "underline", postProcess);
}

const ALIGN_MARKER_RE = /\{:\s*\.(left|center|right)\}\s*$/;

export function textAlignMarkdownPlugin(md: MarkdownIt): void {
  const flagged = md as unknown as { __notturniAlignInstalled?: boolean };
  if (flagged.__notturniAlignInstalled) return;
  flagged.__notturniAlignInstalled = true;

  // eslint-disable-next-line @typescript-eslint/no-explicit-any -- StateCore non è tipizzato dai .d.ts di markdown-it
  md.core.ruler.after("inline", "notturni_text_align", (state: any) => {
    const tokens = state.tokens;
    for (let i = 0; i < tokens.length; i++) {
      const open = tokens[i];
      if (open.type !== "paragraph_open" && open.type !== "heading_open") continue;
      const inline = tokens[i + 1];
      if (!inline || inline.type !== "inline") continue;
      const children = inline.children;
      if (!children || children.length === 0) continue;
      const last = children[children.length - 1];
      if (last.type !== "text") continue;

      const match = ALIGN_MARKER_RE.exec(last.content);
      if (!match) continue;

      const align = match[1];
      const remaining = last.content.slice(0, match.index).replace(/\s+$/, "");
      if (remaining) {
        last.content = remaining;
      } else {
        children.pop();
        const prev = children[children.length - 1];
        if (prev && prev.type === "softbreak") children.pop();
      }
      open.attrSet("style", `text-align: ${align}`);
    }
  });
}
