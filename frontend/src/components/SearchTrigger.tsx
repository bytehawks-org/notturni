"use client";

import { useState, type MouseEvent, type ReactNode } from "react";

import { SearchModal } from "@/components/SearchModal";
import type { SearchScope } from "@/lib/search";

/** Voce "ricerca" dell'header di un blog: senza JavaScript è un link alla pagina
 * `/search` del blog; con un clic semplice apre il modal con l'ambito del blog.
 * Clic con modificatori o rotella lasciano il comportamento del link. */
export function SearchTrigger({
  blogSlug,
  basePath,
  title,
  className,
  children,
}: {
  blogSlug: string;
  basePath: string;
  title?: string;
  className?: string;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  const scope: SearchScope = { kind: "blog", slug: blogSlug, basePath, title };

  function onClick(event: MouseEvent<HTMLAnchorElement>) {
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    event.preventDefault();
    setOpen(true);
  }

  return (
    <>
      {/* Plain anchor: a full navigation without JavaScript, and the global next/link test mock drops handlers. */}
      <a href={`${basePath}/search`} onClick={onClick} aria-haspopup="dialog" className={className}>
        {children}
      </a>
      {/* Mounted only while open, so each opening starts with an empty query and no active result. */}
      {open && <SearchModal open onClose={() => setOpen(false)} scope={scope} />}
    </>
  );
}
