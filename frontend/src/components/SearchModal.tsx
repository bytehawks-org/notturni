"use client";

import { useLocale, useTranslations } from "next-intl";
import { useRouter } from "next/navigation";
import { useEffect, useId, useRef, useState, type FormEvent, type KeyboardEvent, type MouseEvent } from "react";

import { CloseIcon } from "@/components/editor/icons";
import { formatDate } from "@/lib/format";
import { normalizeQuery, resultsHref, SEARCH_MAX_LENGTH, SEARCH_MIN_LENGTH, type SearchScope } from "@/lib/search";
import { useSearchSuggestions } from "@/lib/use-search-suggestions";

const PORTAL_SCOPE: SearchScope = { kind: "portal" };

/** Modal di ricerca aperto dall'icona ⌕ nell'header: `<dialog>` modale con
 * suggerimenti in tempo reale (combobox + listbox). L'invio senza un
 * suggerimento attivo naviga sulla pagina dei risultati dell'ambito. */
export function SearchModal({
  open,
  onClose,
  scope = PORTAL_SCOPE,
}: {
  open: boolean;
  onClose: () => void;
  scope?: SearchScope;
}) {
  const t = useTranslations("SearchPage");
  const tb = useTranslations("BlogSearchPage");
  const tm = useTranslations("SearchModal");
  const tc = useTranslations("Common");
  const locale = useLocale();
  const router = useRouter();
  const [query, setQuery] = useState("");
  const [selection, setSelection] = useState({ key: "", index: -1 });
  const dialogRef = useRef<HTMLDialogElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const pressedBackdrop = useRef(false);
  const id = useId();
  const titleId = `${id}-title`;
  const listId = `${id}-list`;
  const optionId = (index: number) => `${id}-option-${index}`;

  const { status, suggestions } = useSearchSuggestions(scope, query, open);
  const normalized = normalizeQuery(query);
  // La selezione vale solo per l'elenco su cui è stata fatta.
  const resultsKey = suggestions.map((s) => s.post.id).join(" ");
  const activeIndex = selection.key === resultsKey && selection.index < suggestions.length ? selection.index : -1;
  const expanded = suggestions.length > 0;

  const title = scope.kind === "blog" ? tb("title", { blog: scope.title ?? scope.slug }) : t("title");
  const placeholder = scope.kind === "blog" ? tb("placeholder") : t("placeholder");

  useEffect(() => {
    if (!open) return;
    const dialog = dialogRef.current;
    if (!dialog) return;
    const trigger = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const previousOverflow = document.body.style.overflow;
    dialog.showModal();
    inputRef.current?.focus();
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previousOverflow;
      if (dialog.open) dialog.close();
      trigger?.focus();
    };
  }, [open]);

  useEffect(() => {
    if (activeIndex < 0) return;
    document.getElementById(`${id}-option-${activeIndex}`)?.scrollIntoView?.({ block: "nearest" });
  }, [activeIndex, id]);

  if (!open) return null;

  function navigate(href: string) {
    onClose();
    setQuery("");
    setSelection({ key: "", index: -1 });
    router.push(href);
  }

  function onSubmit(event: FormEvent) {
    event.preventDefault();
    if (!normalized) return;
    navigate(resultsHref(scope, normalized));
  }

  function onInputKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    // Durante la composizione IME i tasti appartengono al metodo di input.
    // `keyCode` 229 copre Safari, che chiude la composizione prima del keydown.
    if (event.nativeEvent.isComposing || event.keyCode === 229) {
      if (event.key === "Enter") event.preventDefault();
      return;
    }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      if (!expanded) return;
      event.preventDefault();
      const count = suggestions.length;
      const next =
        event.key === "ArrowDown"
          ? (activeIndex + 1) % count
          : (activeIndex <= 0 ? count : activeIndex) - 1;
      setSelection({ key: resultsKey, index: next });
    } else if (event.key === "Enter" && activeIndex >= 0) {
      event.preventDefault();
      navigate(suggestions[activeIndex].href);
    }
  }

  function onDialogKeyDown(event: KeyboardEvent) {
    if (event.key !== "Escape" || event.nativeEvent.isComposing) return;
    event.preventDefault();
    onClose();
  }

  function onOptionClick(event: MouseEvent, href: string) {
    // Con i tasti modificatori il browser apre il link in una nuova scheda o finestra.
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    event.preventDefault();
    navigate(href);
  }

  function statusMessage(): string {
    switch (status) {
      case "short":
        return tm("typeMore", { min: SEARCH_MIN_LENGTH });
      case "loading":
        return tc("loading");
      case "error":
        return tm("error");
      case "rate_limited":
        return tm("rateLimited");
      case "done":
        if (suggestions.length > 0) return tm("suggestionsCount", { count: suggestions.length });
        return scope.kind === "blog" ? tb("noResults", { query: normalized }) : t("noResults", { query: normalized });
      default:
        return "";
    }
  }

  return (
    <dialog
      ref={dialogRef}
      aria-labelledby={titleId}
      onKeyDown={onDialogKeyDown}
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      // Chiusura decisa dal browser. Un evento in ritardo trova il dialog già riaperto.
      onClose={(event) => {
        if (!event.currentTarget.open) onClose();
      }}
      // Un clic sullo sfondo ha come target il dialog stesso. Il controllo sul
      // `pointerdown` evita di chiudere dopo una selezione di testo trascinata fuori.
      onPointerDown={(event) => {
        pressedBackdrop.current = event.target === event.currentTarget;
      }}
      onClick={(event) => {
        if (pressedBackdrop.current && event.target === event.currentTarget) onClose();
        pressedBackdrop.current = false;
      }}
      className="fixed inset-0 m-0 h-dvh max-h-none w-full max-w-none flex-col overflow-hidden bg-surface p-0 text-foreground backdrop:bg-[rgb(10_10_12/0.55)] open:flex sm:bottom-auto sm:mx-auto sm:mt-[15vh] sm:h-auto sm:max-h-[70dvh] sm:max-w-lg sm:rounded-xl sm:border sm:border-border sm:shadow-soft"
    >
      <form onSubmit={onSubmit} className="flex min-h-0 flex-1 flex-col gap-3 p-5">
        <div className="flex items-center justify-between gap-3">
          <h2 id={titleId} className="min-w-0 truncate font-serif text-lg font-medium tracking-tight">
            {title}
          </h2>
          <button
            type="button"
            onClick={onClose}
            aria-label={tc("close")}
            className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full text-muted hover:bg-background hover:text-foreground [&>svg]:h-4 [&>svg]:w-4"
          >
            <CloseIcon />
          </button>
        </div>
        <div className="flex gap-2">
          <label className="flex min-h-11 min-w-0 flex-1 items-center gap-2 rounded-[10px] border border-border bg-background px-3.5 text-muted focus-within:border-primary focus-within:ring-[3px] focus-within:ring-primary/20">
            <span aria-hidden="true">⌕</span>
            <input
              ref={inputRef}
              name="q"
              type="text"
              role="combobox"
              aria-expanded={expanded}
              aria-controls={listId}
              aria-activedescendant={activeIndex >= 0 ? optionId(activeIndex) : undefined}
              aria-autocomplete="list"
              aria-label={placeholder}
              autoComplete="off"
              enterKeyHint="search"
              maxLength={SEARCH_MAX_LENGTH}
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
                setSelection({ key: "", index: -1 });
              }}
              onKeyDown={onInputKeyDown}
              placeholder={placeholder}
              className="min-h-11 w-full min-w-0 bg-transparent text-base text-foreground outline-none placeholder:text-muted"
            />
          </label>
          <button
            type="submit"
            disabled={!normalized}
            className="min-h-11 shrink-0 rounded-[10px] bg-primary px-4 text-sm font-medium text-background disabled:cursor-not-allowed disabled:opacity-50"
          >
            {scope.kind === "blog" ? tb("searchButton") : t("searchButton")}
          </button>
        </div>
        <p role="status" className="text-sm text-muted empty:-mt-3">
          {statusMessage()}
        </p>
        <div id={listId} role="listbox" aria-label={tm("suggestions")} hidden={!expanded} className="min-h-0 flex-1 overflow-y-auto">
          {suggestions.map(({ post, href }, index) => (
            <a
              key={post.id}
              id={optionId(index)}
              href={href}
              role="option"
              aria-selected={index === activeIndex}
              tabIndex={-1}
              // Il focus resta sull'input.
              onMouseDown={(e) => e.preventDefault()}
              onMouseMove={() => {
                if (index !== activeIndex) setSelection({ key: resultsKey, index });
              }}
              onClick={(e) => onOptionClick(e, href)}
              className="flex min-h-11 flex-col justify-center rounded-[10px] px-3 py-2 text-foreground no-underline aria-selected:bg-background aria-selected:ring-2 aria-selected:ring-inset aria-selected:ring-primary"
            >
              <span className="break-words font-medium">{post.title}</span>
              <span className="flex flex-wrap gap-x-2 text-xs text-muted">
                {scope.kind === "portal" && <span>{post.blog_slug}</span>}
                <span>{formatDate(post.published_at ?? post.created_at, locale, { day: "numeric", month: "short", year: "numeric" })}</span>
              </span>
            </a>
          ))}
        </div>
      </form>
    </dialog>
  );
}
