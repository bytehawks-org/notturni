"use client";

import { createContext, useContext } from "react";

/** Nonce CSP della richiesta corrente (generato in proxy.ts, letto da un
 * Server Component via headers() e propagato qui): serve ai componenti
 * client che iniettano un <Script> proprio (es. il widget Turnstile in
 * CommentsSection.tsx) — script-src usa 'strict-dynamic', che senza nonce
 * ignorerebbe l'allowlist per host e bloccherebbe lo script. */
const NonceContext = createContext<string | null>(null);

export function NonceProvider({ nonce, children }: { nonce: string | null; children: React.ReactNode }) {
  return <NonceContext.Provider value={nonce}>{children}</NonceContext.Provider>;
}

export function useNonce(): string | null {
  return useContext(NonceContext);
}
