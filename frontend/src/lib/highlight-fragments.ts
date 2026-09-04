"use client";

const HIGHLIGHT_CLASS = "fragment-highlight";

/** Stessa normalizzazione applicata lato server (app/domain/fragments.py):
 * spazi bianchi collassati a uno solo, bordi ripuliti. Va usata sia sul testo
 * selezionato dall'utente prima di salvarlo, sia come base per la ricerca in
 * pagina di un frammento già salvato. */
export function normalizeFragmentText(text: string): string {
  return text.replace(/\s+/g, " ").trim();
}

interface TextNodeSpan {
  node: Text;
  start: number;
  end: number;
}

function collectTextNodes(root: HTMLElement): { nodes: TextNodeSpan[]; fullText: string } {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      // un frammento già evidenziato non va ri-considerato come sorgente di
      // testo da abbinare (eviterebbe corrispondenze/annidamenti spuri).
      let el = node.parentElement;
      while (el && el !== root) {
        if (el.classList.contains(HIGHLIGHT_CLASS)) return NodeFilter.FILTER_REJECT;
        el = el.parentElement;
      }
      return NodeFilter.FILTER_ACCEPT;
    },
  });

  const nodes: TextNodeSpan[] = [];
  let offset = 0;
  let current: Node | null = walker.nextNode();
  while (current) {
    const text = current as Text;
    nodes.push({ node: text, start: offset, end: offset + text.data.length });
    offset += text.data.length;
    current = walker.nextNode();
  }
  return { nodes, fullText: nodes.map((n) => n.node.data).join("") };
}

function locate(nodes: TextNodeSpan[], pos: number): { node: Text; offset: number } | null {
  for (const span of nodes) {
    if (pos <= span.end) return { node: span.node, offset: pos - span.start };
  }
  return null;
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/** Il testo normalizzato (spazi già collassati a uno) va confrontato contro
 * il testo "grezzo" dei nodi DOM, dove uno spazio nel sorgente Markdown può
 * essere un a-capo o più spazi consecutivi: ogni spazio del testo normalizzato
 * diventa quindi `\s+` nel pattern di ricerca. */
function toMatcher(normalizedText: string): RegExp {
  const words = normalizedText.split(" ").filter(Boolean).map(escapeRegExp);
  return new RegExp(words.join("\\s+"));
}

/** Cerca ed evidenzia (con <mark class="fragment-highlight">) i frammenti già
 * salvati dall'utente su questo post, ad ogni lettura — indipendentemente dal
 * fatto che si sia arrivati dalla pagina di raccolta. Ricerca per
 * corrispondenza testuale (non un offset salvato): tollera piccoli spazi
 * bianchi diversi, ma un frammento non più trovabile (es. post modificato
 * dall'autore dopo il salvataggio) semplicemente non viene ri-evidenziato,
 * senza errori.
 *
 * Limite noto: una selezione salvata a cavallo di due blocchi (es. due
 * paragrafi distinti) potrebbe non ri-evidenziarsi, perché qui i nodi di
 * testo vengono concatenati senza un separatore ai confini di blocco — a
 * differenza di `Selection.toString()` (usato al salvataggio), che inserisce
 * un a-capo in quel punto. Non compromette il salvataggio, solo la
 * ri-evidenziazione in casi limite. */
export function highlightFragments(root: HTMLElement, fragments: { id: string; text: string }[]): void {
  if (fragments.length === 0) return;
  const { nodes, fullText } = collectTextNodes(root);
  if (nodes.length === 0) return;

  const claimed: { start: number; end: number }[] = [];
  const toWrap: { id: string; start: number; end: number }[] = [];

  for (const fragment of fragments) {
    const normalized = normalizeFragmentText(fragment.text);
    if (!normalized) continue;
    const match = toMatcher(normalized).exec(fullText);
    if (!match) continue;
    const start = match.index;
    const end = start + match[0].length;
    if (claimed.some((c) => start < c.end && end > c.start)) continue;
    claimed.push({ start, end });
    toWrap.push({ id: fragment.id, start, end });
  }

  // Dal fondo del testo verso l'inizio: il wrapping di un intervallo può
  // dividere il nodo di testo al suo interno (Range.extractContents), ma
  // tocca solo quel punto in poi — i riferimenti calcolati sopra per gli
  // intervalli precedenti (offset minori, mai sovrapposti) restano validi.
  toWrap.sort((a, b) => b.start - a.start);

  for (const { id, start, end } of toWrap) {
    const from = locate(nodes, start);
    const to = locate(nodes, end);
    if (!from || !to) continue;

    const range = document.createRange();
    range.setStart(from.node, from.offset);
    range.setEnd(to.node, to.offset);

    const mark = document.createElement("mark");
    mark.className = HIGHLIGHT_CLASS;
    mark.dataset.fragmentId = id;
    mark.appendChild(range.extractContents());
    range.insertNode(mark);
  }
}

/** Id dei frammenti già evidenziati che la selezione corrente tocca, anche
 * solo in parte — usato per proporre "Rimuovi frammento" invece di "Salva
 * frammento" nel menu contestuale quando si ri-seleziona un testo già
 * salvato. */
export function findOverlappingFragmentIds(root: HTMLElement, range: Range): string[] {
  const marks = root.querySelectorAll<HTMLElement>(`mark.${HIGHLIGHT_CLASS}`);
  const ids = new Set<string>();
  marks.forEach((mark) => {
    if (range.intersectsNode(mark) && mark.dataset.fragmentId) ids.add(mark.dataset.fragmentId);
  });
  return [...ids];
}

/** Toglie l'evidenziazione di un frammento rimosso, riportando il suo testo
 * a nodo di testo semplice (di nuovo disponibile per una ricerca futura, se
 * il frammento venisse risalvato). */
export function unwrapFragmentMark(root: HTMLElement, fragmentId: string): void {
  const mark = root.querySelector<HTMLElement>(`mark.${HIGHLIGHT_CLASS}[data-fragment-id="${fragmentId}"]`);
  if (!mark) return;
  const parent = mark.parentNode;
  if (!parent) return;
  while (mark.firstChild) parent.insertBefore(mark.firstChild, mark);
  parent.removeChild(mark);
  parent.normalize();
}
