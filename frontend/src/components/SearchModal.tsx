"use client";

import { useTranslations } from "next-intl";
import { useRouter } from "next/navigation";
import { useEffect, useState, type FormEvent } from "react";

import { CloseIcon } from "@/components/editor/icons";

/** Modal di ricerca aperto dall'icona ⌕ in SiteHeader: mantiene stile/overlay
 * degli altri dialoghi del sito (Lightbox, NoteDialog) invece di navigare
 * subito su una pagina dedicata. L'invio del form naviga su `/search?q=`,
 * dove i risultati restano una pagina a sé (CLAUDE.md, richiesta utente). */
export function SearchModal({ open, onClose }: { open: boolean; onClose: () => void }) {
  const t = useTranslations("SearchPage");
  const tc = useTranslations("Common");
  const router = useRouter();
  const [query, setQuery] = useState("");

  useEffect(() => {
    if (!open) return;
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    document.addEventListener("keydown", onKeyDown);
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.body.style.overflow = previousOverflow;
    };
  }, [open, onClose]);

  if (!open) return null;

  function onSubmit(event: FormEvent) {
    event.preventDefault();
    const trimmed = query.trim();
    if (!trimmed) return;
    onClose();
    setQuery("");
    router.push(`/search?q=${encodeURIComponent(trimmed)}`);
  }

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={t("title")}
      onClick={onClose}
      className="fixed inset-0 z-[100] grid place-items-start bg-[rgb(10_10_12/0.55)] p-4 pt-[15vh]"
    >
      <form
        onClick={(e) => e.stopPropagation()}
        onSubmit={onSubmit}
        className="mx-auto flex w-full max-w-lg flex-col gap-3 rounded-xl border border-border bg-surface p-5 shadow-soft"
      >
        <div className="flex items-center justify-between">
          <h2 className="font-serif text-lg font-medium tracking-tight">{t("title")}</h2>
          <button
            type="button"
            onClick={onClose}
            aria-label={tc("close")}
            className="flex h-8 w-8 items-center justify-center rounded-full text-muted hover:bg-background hover:text-foreground [&>svg]:h-4 [&>svg]:w-4"
          >
            <CloseIcon />
          </button>
        </div>
        <label className="flex items-center gap-2 rounded-[10px] border border-border bg-background px-3.5 py-2.5 text-sm text-muted focus-within:border-primary focus-within:ring-[3px] focus-within:ring-primary/20">
          <span aria-hidden="true" className="text-base">⌕</span>
          <input
            name="q"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t("placeholder")}
            aria-label={t("placeholder")}
            autoFocus
            className="w-full bg-transparent text-foreground outline-none placeholder:text-muted"
          />
        </label>
      </form>
    </div>
  );
}
