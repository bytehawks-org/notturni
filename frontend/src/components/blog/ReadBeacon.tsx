"use client";

import { useEffect } from "react";

import { apiUrl } from "@/lib/site";

const READ_DELAY_MS = 8000;

/** Mockup 5a "Reads": dopo alcuni secondi sulla pagina invia `POST
 * /posts/{id}/read` con `sendBeacon` (nessun cookie, nessun identificativo:
 * il backend salva solo +1 sul giorno). Non fa nulla se la scheda viene
 * chiusa prima del ritardo. */
export function ReadBeacon({ postId }: { postId: string }) {
  useEffect(() => {
    const timer = window.setTimeout(() => {
      const url = `${apiUrl()}/api/v1/posts/${postId}/read`;
      try {
        if (!navigator.sendBeacon || !navigator.sendBeacon(url)) {
          void fetch(url, { method: "POST", keepalive: true }).catch(() => undefined);
        }
      } catch {
        // nessun feedback: il conteggio è best-effort
      }
    }, READ_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [postId]);
  return null;
}
