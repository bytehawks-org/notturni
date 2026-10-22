"use client";

import { useEffect } from "react";

const COPIED_RESET_MS = 2000;

/** Tasto "copia" sui blocchi di codice dei post pubblici (blocco
 * "evidenziazione sintassi"): stessa delega globale di `LightboxProvider`
 * per lo stesso motivo — il contenuto dei post arriva via
 * `dangerouslySetInnerHTML` (lib/markdown.ts), niente handler React per
 * ciascun blocco. Il pulsante e il testo da copiare (`data-copy-text`) sono
 * già nel markup renderizzato server-side; qui solo il click-to-clipboard e
 * il feedback visivo (`data-copied`, stile in globals.css). Montato una
 * sola volta alla radice (app/layout.tsx), come LightboxProvider. */
export function CodeCopyProvider() {
  useEffect(() => {
    function onClick(event: MouseEvent) {
      const target = event.target as HTMLElement | null;
      const button = target?.closest<HTMLElement>(".code-copy-button");
      if (!button) return;
      const text = button.getAttribute("data-copy-text") ?? "";
      const copyLabel = button.getAttribute("data-label-copy") ?? button.textContent ?? "";
      const copiedLabel = button.getAttribute("data-label-copied") ?? copyLabel;
      navigator.clipboard
        .writeText(text)
        .then(() => {
          button.setAttribute("data-copied", "true");
          button.textContent = copiedLabel;
          window.setTimeout(() => {
            button.removeAttribute("data-copied");
            button.textContent = copyLabel;
          }, COPIED_RESET_MS);
        })
        .catch(() => undefined);
    }
    document.addEventListener("click", onClick);
    return () => document.removeEventListener("click", onClick);
  }, []);

  return null;
}
