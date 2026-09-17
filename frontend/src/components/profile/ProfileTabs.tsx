"use client";

import type { ReactNode } from "react";
import { useState } from "react";

/** Tab Post/Blog/Commenti del profilo pubblico (mockup 3e). Il contenuto di
 * ogni tab arriva già renderizzato dal Server Component chiamante (nessuna
 * fetch qui): tutti e tre restano nell'HTML iniziale (buono anche per
 * l'indicizzazione), lo switch nasconde/mostra con `hidden` invece di
 * montare/smontare. */
export function ProfileTabs({
  tabs,
}: {
  tabs: { id: string; label: string; count: number; content: ReactNode }[];
}) {
  const [active, setActive] = useState(tabs[0]?.id);

  return (
    <section className="flex flex-col gap-4">
      <div className="flex gap-1 border-b border-border">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            onClick={() => setActive(tab.id)}
            className={`border-b-2 px-3.5 py-2 text-sm transition ${
              active === tab.id ? "border-primary font-medium text-foreground" : "border-transparent text-muted hover:text-foreground"
            }`}
          >
            {tab.label}
            <span className="ml-1 text-muted">· {tab.count}</span>
          </button>
        ))}
      </div>
      {tabs.map((tab) => (
        <div key={tab.id} hidden={active !== tab.id}>
          {tab.content}
        </div>
      ))}
    </section>
  );
}
