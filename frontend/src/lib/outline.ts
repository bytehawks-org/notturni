/**
 * Operazioni sull'indice di uno Spazio wiki/docs. L'indice è un elenco piatto in ordine di lettura
 * (depth-first) con la profondità di ogni voce: spostare, indentare e de-indentare sono manipolazioni
 * di intervalli (una voce con tutte le sue sottopagine), e il padre di ciascuna si ricava dalle
 * profondità. Funzioni pure: restituiscono un nuovo elenco, o lo stesso se l'operazione non è possibile.
 */

export interface OutlineEntry {
  id: string;
  depth: number;
}

/** Fine (esclusa) della voce `i` con le sue sottopagine. */
function subtreeEnd<T extends OutlineEntry>(list: T[], i: number): number {
  let j = i + 1;
  while (j < list.length && list[j].depth > list[i].depth) j++;
  return j;
}

/** Indice del fratello precedente della voce `i`, se esiste. */
function previousSibling<T extends OutlineEntry>(list: T[], i: number): number | null {
  for (let k = i - 1; k >= 0; k--) {
    if (list[k].depth < list[i].depth) return null;
    if (list[k].depth === list[i].depth) return k;
  }
  return null;
}

export function moveUp<T extends OutlineEntry>(list: T[], i: number): T[] {
  const prev = previousSibling(list, i);
  if (prev === null) return list;
  const end = subtreeEnd(list, i);
  return [...list.slice(0, prev), ...list.slice(i, end), ...list.slice(prev, i), ...list.slice(end)];
}

export function moveDown<T extends OutlineEntry>(list: T[], i: number): T[] {
  const end = subtreeEnd(list, i);
  if (end >= list.length || list[end].depth !== list[i].depth) return list;
  const nextEnd = subtreeEnd(list, end);
  return [...list.slice(0, i), ...list.slice(end, nextEnd), ...list.slice(i, end), ...list.slice(nextEnd)];
}

/** La voce diventa l'ultima figlia del fratello che la precede. */
export function indent<T extends OutlineEntry>(list: T[], i: number, maxDepth: number): T[] {
  if (previousSibling(list, i) === null) return list;
  const end = subtreeEnd(list, i);
  const depthOf = Math.max(...list.slice(i, end).map((e) => e.depth)) - list[i].depth;
  if (list[i].depth + 1 + depthOf > maxDepth) return list;
  return list.map((e, k) => (k >= i && k < end ? { ...e, depth: e.depth + 1 } : e));
}

/** La voce passa al livello del padre, subito dopo di lui (le sorelle successive restano sotto il padre). */
export function outdent<T extends OutlineEntry>(list: T[], i: number): T[] {
  const depth = list[i].depth;
  if (depth === 0) return list;
  const end = subtreeEnd(list, i);
  const moved = list.slice(i, end).map((e) => ({ ...e, depth: e.depth - 1 }));
  const rest = [...list.slice(0, i), ...list.slice(end)];
  let parent = i - 1;
  while (rest[parent].depth >= depth) parent--;
  const insertAt = subtreeEnd(rest, parent);
  return [...rest.slice(0, insertAt), ...moved, ...rest.slice(insertAt)];
}

/** `{id, parent_id}` nell'ordine dell'elenco, come richiede `PUT /blogs/{slug}/outline`. */
export function toItems(list: OutlineEntry[]): { id: string; parent_id: string | null }[] {
  const stack: string[] = [];
  return list.map((e) => {
    stack.length = e.depth;
    const parent = e.depth > 0 ? stack[e.depth - 1] ?? null : null;
    stack[e.depth] = e.id;
    return { id: e.id, parent_id: parent };
  });
}

/** Id della voce `i` e di tutte le sue sottopagine (per non offrirle come padre di sé stessa). */
export function subtreeIds(list: OutlineEntry[], i: number): Set<string> {
  return new Set(list.slice(i, subtreeEnd(list, i)).map((e) => e.id));
}
