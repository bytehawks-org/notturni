"use client";

import { useEffect, useState } from "react";

import { api } from "@/lib/api";
import { useSiteConfigValue } from "@/components/SiteConfigProvider";

/** Richiesta condivisa di `GET /api/v1/config`: una sola chiamata per pagina, anche con molte card. */
let cached: Promise<string | null> | null = null;

/** Slug del blog primario (solo). Il valore arriva dal `SiteConfigProvider` (letto sul server nel layout), quindi
 * è già presente al primo render, anche in SSR. Il fetch di `api.config.get()` resta solo come ripiego: senza
 * provider, oppure in solo quando il provider non ha lo slug. In platform non si fa nessuna richiesta. `null`
 * se non c'è un primario o la richiesta fallisce. Solo per i client component: sul server usare
 * `getPrimaryBlogSlug()` (`@/lib/server-api`). */
export function usePrimaryBlogSlug(): string | null {
  const config = useSiteConfigValue();
  const fromProvider = config?.primarySlug ?? null;
  const needsFetch = !config || (config.deploymentMode === "solo" && !fromProvider);
  const [fetched, setFetched] = useState<string | null>(null);
  useEffect(() => {
    if (!needsFetch) return;
    let active = true;
    try {
      cached ??= api.config
        .get()
        .then((config) => config.primary_blog_slug ?? null)
        .catch(() => {
          cached = null;
          return null;
        });
    } catch {
      cached = null;
      return;
    }
    cached.then((value) => {
      if (active) setFetched(value);
    });
    return () => {
      active = false;
    };
  }, [needsFetch]);
  return fromProvider ?? (needsFetch ? fetched : null);
}
