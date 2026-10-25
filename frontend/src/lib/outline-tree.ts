import type { OutlineNode } from "./types";

/** Antenati di `id` dalla radice fino al padre diretto (vuoto per una radice o un id sconosciuto). */
export function ancestorsOf(nodes: OutlineNode[], id: string): OutlineNode[] {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const chain: OutlineNode[] = [];
  const seen = new Set<string>([id]);
  for (let cur = byId.get(id)?.parent_id; cur && !seen.has(cur); cur = byId.get(cur)?.parent_id) {
    seen.add(cur);
    const node = byId.get(cur);
    if (!node) break;
    chain.unshift(node);
  }
  return chain;
}

/** Pagina precedente e successiva in ordine di lettura (depth-first), come nei manuali. */
export function neighbours(nodes: OutlineNode[], id: string): { prev: OutlineNode | null; next: OutlineNode | null } {
  const i = nodes.findIndex((n) => n.id === id);
  if (i < 0) return { prev: null, next: null };
  return { prev: nodes[i - 1] ?? null, next: nodes[i + 1] ?? null };
}

/** Figli diretti di `id` (o le radici se `null`), nell'ordine dell'indice. */
export function childrenOf(nodes: OutlineNode[], id: string | null): OutlineNode[] {
  return nodes.filter((n) => n.parent_id === id);
}
