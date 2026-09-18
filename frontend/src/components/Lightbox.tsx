"use client";

import { useTranslations } from "next-intl";
import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

import { CloseIcon } from "@/components/editor/icons";

interface LightboxState {
  src: string;
  alt: string;
}

const LightboxContext = createContext<((src: string, alt?: string) => void) | null>(null);

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
  const t = useTranslations("Common");
  const [state, setState] = useState<LightboxState | null>(null);

  const open = useCallback((src: string, alt: string = "") => setState({ src, alt }), []);
  const close = useCallback(() => setState(null), []);

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
        if (src) open(src, expandBtn.getAttribute("data-lightbox-alt") ?? "");
        return;
      }

      const img = target.closest<HTMLElement>("img[data-lightbox]");
      if (img) {
        // Un'immagine di contenuto può trovarsi dentro un link Markdown
        // (`[![alt](src)](href)`): senza questo lo stesso click apre la
        // lightbox *e* segue il link, portando via dal post.
        event.preventDefault();
        open(img.getAttribute("src") ?? "", img.getAttribute("alt") ?? "");
      }
    }
    document.addEventListener("click", onClick);
    return () => document.removeEventListener("click", onClick);
  }, [open]);

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
        open(img.getAttribute("src") ?? "", img.getAttribute("alt") ?? "");
      }
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [open]);

  useEffect(() => {
    if (!state) return;
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") close();
    }
    document.addEventListener("keydown", onKeyDown);
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      document.body.style.overflow = previousOverflow;
    };
  }, [state, close]);

  const value = useMemo(() => open, [open]);

  return (
    <LightboxContext.Provider value={value}>
      {children}
      {state && (
        <div
          role="dialog"
          aria-modal="true"
          onClick={close}
          className="fixed inset-0 z-[100] flex items-center justify-center bg-black/90 p-6"
        >
          <button
            type="button"
            onClick={close}
            aria-label={t("close")}
            className="absolute right-4 top-4 flex h-10 w-10 items-center justify-center rounded-full text-white/80 hover:bg-white/10 hover:text-white [&>svg]:h-5 [&>svg]:w-5"
          >
            <CloseIcon />
          </button>
          {/* eslint-disable-next-line @next/next/no-img-element -- URL storage esterno arbitrario */}
          <img
            src={state.src}
            alt={state.alt}
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
