"use client";

import { useTranslations } from "next-intl";
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import { CloseIcon } from "@/components/editor/icons";

interface LightboxItem {
  src: string;
  alt: string;
}

interface LightboxState {
  items: LightboxItem[];
  index: number;
  opener: HTMLElement | null;
}

const LightboxContext = createContext<((src: string, alt?: string) => void) | null>(null);

const SWIPE_THRESHOLD_PX = 50;

/** Raccoglie le immagini del gruppo gallery di `el` (immagine o pulsante di
 * ingrandimento). Gli id di gruppo sono contatori per-render (g1, g2...) e
 * possono collidere tra post nello stesso documento: si limita la ricerca al
 * `.post-gallery` più vicino. Policy per le tile sensibili: restano nella
 * navigazione solo se già rivelate (checkbox spuntato); quelle nascoste
 * vengono saltate, così la lightbox non mostra mai un contenuto non
 * richiesto. Ritorna null se `el` non è in una gallery. */
function collectGroup(el: HTMLElement, src: string): { items: LightboxItem[]; index: number } | null {
  const container = el.closest<HTMLElement>(".post-gallery");
  const gid = el.getAttribute("data-gallery");
  if (!container || !gid) return null;
  const own = Number(el.getAttribute("data-gallery-index"));
  const imgs = Array.from(container.querySelectorAll<HTMLImageElement>("img[data-gallery]"))
    .filter((i) => i.getAttribute("data-gallery") === gid)
    .filter((i) => {
      if (Number(i.getAttribute("data-gallery-index")) === own) return true;
      const wrapper = i.closest(".sensitive-image-wrapper");
      if (!wrapper) return true;
      return !!wrapper.querySelector<HTMLInputElement>(".sensitive-image-toggle")?.checked;
    })
    .sort((x, y) => Number(x.getAttribute("data-gallery-index")) - Number(y.getAttribute("data-gallery-index")));
  const index = imgs.findIndex((i) => Number(i.getAttribute("data-gallery-index")) === own);
  if (index < 0) return null;
  const items = imgs.map((i) => ({ src: i.getAttribute("src") ?? "", alt: i.getAttribute("alt") ?? "" }));
  items[index] = { src: src || items[index].src, alt: items[index].alt };
  return { items, index };
}

/**
 * Rifinitura #1: lightbox per le immagini pubblicate. Montato una sola volta
 * alla radice (app/layout.tsx) così un solo overlay serve tutte le pagine
 * pubbliche (blog, post, media) senza duplicare stato.
 *
 * Le immagini non segnalate sono cliccabili direttamente (attributo
 * `data-lightbox` sull'`<img>`, vedi lib/markdown.ts e CoverImage.tsx). Le
 * immagini segnalate come sensibili restano un problema a parte: il primo
 * click/tap le rivela (trucco CSS label→checkbox esistente, vedi
 * `.sensitive-image-wrapper` in globals.css) e solo dopo compare, in alto a
 * destra, il pulsante `.lightbox-expand-btn` per ingrandirle — mai le due
 * cose insieme sullo stesso click, altrimenti chi non vuole vedere
 * l'immagine se la troverebbe comunque ingrandita in faccia.
 */
export function LightboxProvider({ children }: { children: ReactNode }) {
  const tl = useTranslations("Lightbox");
  const tg = useTranslations("Gallery");
  const [state, setState] = useState<LightboxState | null>(null);
  const dialogRef = useRef<HTMLDivElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  const touchX = useRef<number | null>(null);

  const openFrom = useCallback((src: string, alt: string, opener: HTMLElement | null, group?: HTMLElement) => {
    const g = group ? collectGroup(group, src) : null;
    setState({ items: g?.items ?? [{ src, alt }], index: g?.index ?? 0, opener });
  }, []);
  const open = useCallback(
    (src: string, alt: string = "") => openFrom(src, alt, document.activeElement as HTMLElement | null),
    [openFrom],
  );
  const stateRef = useRef<LightboxState | null>(null);
  useEffect(() => {
    stateRef.current = state;
  }, [state]);
  const close = useCallback(() => {
    stateRef.current?.opener?.focus?.();
    setState(null);
  }, []);
  const go = useCallback((to: number | "first" | "last" | "prev" | "next") => {
    setState((s) => {
      if (!s) return s;
      const last = s.items.length - 1;
      const raw = to === "first" ? 0 : to === "last" ? last : to === "prev" ? s.index - 1 : to === "next" ? s.index + 1 : to;
      const index = Math.min(last, Math.max(0, raw));
      return index === s.index ? s : { ...s, index };
    });
  }, []);

  // Delega globale invece di un listener per immagine: sia il contenuto dei
  // post (HTML iniettato via dangerouslySetInnerHTML in FragmentReader) sia
  // le cover renderizzate da Server Component passano dagli stessi due
  // marcatori (`[data-lightbox]` sull'immagine, `.lightbox-expand-btn` sul
  // pulsante) senza bisogno di un handler React per ciascuna.
  useEffect(() => {
    function onClick(event: MouseEvent) {
      const target = event.target as HTMLElement | null;
      if (!target) return;

      const expandBtn = target.closest<HTMLElement>(".lightbox-expand-btn");
      if (expandBtn) {
        // dentro un <label> (trucco sensitive-image-wrapper): senza questo,
        // il click continuerebbe a "bollare" anche il toggle del checkbox.
        event.preventDefault();
        event.stopPropagation();
        const src = expandBtn.getAttribute("data-lightbox-src");
        if (src) openFrom(src, expandBtn.getAttribute("data-lightbox-alt") ?? "", expandBtn, expandBtn);
        return;
      }

      const img = target.closest<HTMLElement>("img[data-lightbox]");
      if (img) {
        // Un'immagine di contenuto può trovarsi dentro un link Markdown
        // (`[![alt](src)](href)`): senza questo lo stesso click apre la
        // lightbox *e* segue il link, portando via dal post.
        event.preventDefault();
        openFrom(img.getAttribute("src") ?? "", img.getAttribute("alt") ?? "", img, img);
      }
    }
    document.addEventListener("click", onClick);
    return () => document.removeEventListener("click", onClick);
  }, [openFrom]);

  // Le stesse immagini sono raggiungibili da tastiera (tabIndex sull'img,
  // impostato in lib/markdown.ts/CoverImage) — qui basta intercettare
  // Invio/Spazio con la stessa delega globale del click, senza un handler
  // per immagine.
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "Enter" && event.key !== " ") return;
      const target = event.target as HTMLElement | null;
      const img = target?.closest<HTMLElement>("img[data-lightbox]");
      if (img) {
        event.preventDefault();
        openFrom(img.getAttribute("src") ?? "", img.getAttribute("alt") ?? "", img, img);
      }
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [openFrom]);

  const isOpen = state !== null;
  useEffect(() => {
    if (!isOpen) return;
    closeRef.current?.focus();
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        close();
      } else if (event.key === "ArrowLeft") {
        go("prev");
      } else if (event.key === "ArrowRight") {
        go("next");
      } else if (event.key === "Home") {
        event.preventDefault();
        go("first");
      } else if (event.key === "End") {
        event.preventDefault();
        go("last");
      } else if (event.key === "Tab") {
        const focusable = Array.from(
          dialogRef.current?.querySelectorAll<HTMLElement>("button:not([disabled])") ?? [],
        );
        if (focusable.length === 0) return;
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        const active = document.activeElement;
        if (!dialogRef.current?.contains(active)) {
          event.preventDefault();
          first.focus();
        } else if (event.shiftKey && active === first) {
          event.preventDefault();
          last.focus();
        } else if (!event.shiftKey && active === last) {
          event.preventDefault();
          first.focus();
        }
      }
    }
    document.addEventListener("keydown", onKeyDown);
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.body.style.overflow = previousOverflow;
    };
  }, [isOpen, close, go]);

  const value = useMemo(() => open, [open]);

  return (
    <LightboxContext.Provider value={value}>
      {children}
      {state && (
        <div
          ref={dialogRef}
          role="dialog"
          aria-modal="true"
          aria-label={state.items[state.index].alt || tg("label")}
          onClick={close}
          onTouchStart={(e) => {
            touchX.current = e.touches[0]?.clientX ?? null;
          }}
          onTouchEnd={(e) => {
            const start = touchX.current;
            touchX.current = null;
            const end = e.changedTouches[0]?.clientX;
            if (start === null || end === undefined) return;
            const dx = end - start;
            if (Math.abs(dx) >= SWIPE_THRESHOLD_PX) go(dx < 0 ? "next" : "prev");
          }}
          className="fixed inset-0 z-[100] flex items-center justify-center bg-black/90 p-6"
        >
          <button
            ref={closeRef}
            type="button"
            onClick={close}
            aria-label={tl("close")}
            className="absolute right-4 top-4 flex h-10 w-10 items-center justify-center rounded-full text-white/80 hover:bg-white/10 hover:text-white [&>svg]:h-5 [&>svg]:w-5"
          >
            <CloseIcon />
          </button>
          {state.items.length > 1 && (
            <>
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  go("prev");
                }}
                disabled={state.index === 0}
                aria-label={tl("prev")}
                className="absolute left-4 top-1/2 flex h-10 w-10 -translate-y-1/2 items-center justify-center rounded-full text-xl text-white/80 hover:bg-white/10 hover:text-white disabled:opacity-30"
              >
                <span aria-hidden="true">&#8249;</span>
              </button>
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  go("next");
                }}
                disabled={state.index === state.items.length - 1}
                aria-label={tl("next")}
                className="absolute right-4 top-1/2 flex h-10 w-10 -translate-y-1/2 items-center justify-center rounded-full text-xl text-white/80 hover:bg-white/10 hover:text-white disabled:opacity-30"
              >
                <span aria-hidden="true">&#8250;</span>
              </button>
              <p
                aria-live="polite"
                className="absolute bottom-4 left-1/2 -translate-x-1/2 text-sm text-white/80"
              >
                {tl("counter", { current: state.index + 1, total: state.items.length })}
              </p>
            </>
          )}
          {/* eslint-disable-next-line @next/next/no-img-element -- URL storage esterno arbitrario */}
          <img
            src={state.items[state.index].src}
            alt={state.items[state.index].alt}
            onClick={(e) => e.stopPropagation()}
            className="max-h-[90vh] max-w-full rounded-md object-contain shadow-2xl"
          />
        </div>
      )}
    </LightboxContext.Provider>
  );
}

/** `open(src, alt?)`: apre la lightbox su un'immagine. Usato solo da
 * CoverImage — il resto (immagini di contenuto) passa dalla delega globale
 * sopra, senza dover risalire al context da un Server Component. */
export function useLightbox(): (src: string, alt?: string) => void {
  const ctx = useContext(LightboxContext);
  if (!ctx) throw new Error("useLightbox deve essere usato dentro LightboxProvider");
  return ctx;
}
