"use client";

import { useEffect, useState } from "react";

import { api } from "./api";

/** Funzioni sperimentali attive sull'istanza (`GET /config`). Finché la risposta non arriva, o se
 * fallisce, l'elenco è vuoto: l'interfaccia offre solo ciò che è sempre disponibile. */
export function useExperimentalFeatures(): string[] {
  const [features, setFeatures] = useState<string[]>([]);
  useEffect(() => {
    let cancelled = false;
    api.config
      .get()
      .then((config) => {
        if (!cancelled) setFeatures(config.experimental_features ?? []);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, []);
  return features;
}
