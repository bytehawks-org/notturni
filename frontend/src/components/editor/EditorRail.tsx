"use client";

import { useState, type ReactNode } from "react";

export interface EditorRailTab {
  id: string;
  label: string;
  badge?: string | number;
  content: ReactNode;
}

/**
 * Rail destro dell'editor (mockup 1d, 300px): raccoglie i controlli del post
 * sotto tab invece di accumularli tutti sopra la toolbar di formattazione.
 * Un solo tab non mostra la barra (usato da "Nuovo post", dove non esistono
 * ancora traduzioni).
 */
export function EditorRail({ tabs }: { tabs: EditorRailTab[] }) {
  const [activeId, setActiveId] = useState(tabs[0]?.id);
  const active = tabs.find((t) => t.id === activeId) ?? tabs[0];

  return (
    <aside className="flex flex-col gap-5 border-border pt-8 lg:border-l lg:pl-6 lg:pt-0">
      {tabs.length > 1 && (
        <div className="flex gap-4 border-b border-border pb-2.5 text-[13px] text-muted">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              type="button"
              onClick={() => setActiveId(tab.id)}
              className={tab.id === active?.id ? "font-semibold text-foreground" : "hover:text-foreground"}
            >
              {tab.label}
              {tab.badge !== undefined && <span className="ml-1 text-muted">· {tab.badge}</span>}
            </button>
          ))}
        </div>
      )}
      <div className="flex flex-col gap-5 text-sm">{active?.content}</div>
    </aside>
  );
}
