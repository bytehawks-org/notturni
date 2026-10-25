import Link from "next/link";

import type { OutlineNode } from "@/lib/types";

/** Indice laterale di uno Spazio wiki/docs: l'elenco dell'outline con rientro per livello. La
 * pagina corrente è evidenziata; le sezioni che non contengono la pagina corrente si possono
 * chiudere (`<details>`, nessun JavaScript). Server component. */
export function SpaceNav({
  nodes,
  currentId,
  hrefFor,
  openPath,
}: {
  nodes: OutlineNode[];
  currentId?: string;
  hrefFor: (node: OutlineNode) => string;
  /** Id degli antenati della pagina corrente: restano aperti. */
  openPath: Set<string>;
}) {
  const children = new Map<string | null, OutlineNode[]>();
  const known = new Set(nodes.map((n) => n.id));
  for (const n of nodes) {
    const parent = n.parent_id && known.has(n.parent_id) ? n.parent_id : null;
    children.set(parent, [...(children.get(parent) ?? []), n]);
  }

  function renderLevel(parent: string | null, level: number) {
    const list = children.get(parent);
    if (!list) return null;
    return (
      <ul className={level === 0 ? "flex flex-col gap-0.5" : "mt-0.5 flex flex-col gap-0.5 border-l border-border pl-3"}>
        {list.map((n) => {
          const current = n.id === currentId;
          const link = (
            <Link
              href={hrefFor(n)}
              aria-current={current ? "page" : undefined}
              className={`block rounded-md px-2 py-1 text-[14px] no-underline transition ${
                current ? "bg-primary/10 font-medium text-primary" : "text-foreground/80 hover:bg-surface hover:text-foreground"
              }`}
            >
              {n.title}
            </Link>
          );
          const sub = renderLevel(n.id, level + 1);
          return (
            <li key={n.id}>
              {sub ? (
                <details open={openPath.has(n.id) || current || level === 0}>
                  <summary className="cursor-pointer list-none [&::-webkit-details-marker]:hidden">{link}</summary>
                  {sub}
                </details>
              ) : (
                link
              )}
            </li>
          );
        })}
      </ul>
    );
  }

  return <nav aria-label="Indice">{renderLevel(null, 0)}</nav>;
}
