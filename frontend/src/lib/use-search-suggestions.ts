"use client";

import { useEffect, useState } from "react";
import { ApiClientError, api } from "./api";
import {
  normalizeQuery,
  resultHref,
  SEARCH_DEBOUNCE_MS,
  SEARCH_MIN_LENGTH,
  SEARCH_SUGGESTION_LIMIT,
  type SearchScope,
} from "./search";
import type { Post } from "./types";

export type SearchSuggestionStatus = "idle" | "short" | "loading" | "done" | "error" | "rate_limited";

export interface SearchSuggestion {
  post: Post;
  href: string;
}

export interface SearchSuggestionsState {
  status: SearchSuggestionStatus;
  suggestions: SearchSuggestion[];
}

interface Settled {
  key: string;
  status: "done" | "error" | "rate_limited";
  posts: Post[];
}

/**
 * Suggerimenti di ricerca per l'ambito dato (piattaforma o singolo blog).
 *
 * - `query` vuota: `idle`; sotto `SEARCH_MIN_LENGTH` caratteri: `short`, nessuna richiesta.
 * - Dopo `SEARCH_DEBOUNCE_MS` dall'ultimo cambio parte una sola richiesta, con `AbortSignal`.
 * - Un cambio di query o di ambito, o `enabled=false`, annulla la richiesta in corso e il timer.
 * - Le risposte di query precedenti non vengono mai mostrate: lo stato è legato alla chiave query+ambito.
 * - Nessun token o cookie: la ricerca usa solo i contenuti pubblici.
 *
 * Gli stati `loading` e `idle` sono ricavati nel render, non memorizzati.
 */
export function useSearchSuggestions(
  scope: SearchScope,
  rawQuery: string,
  enabled = true,
): SearchSuggestionsState {
  const slug = scope.kind === "blog" ? scope.slug : null;
  const query = normalizeQuery(rawQuery);
  const key = `${slug ?? "portal"}\u0000${query}`;
  const [settled, setSettled] = useState<Settled | null>(null);

  const active = enabled && query.length >= SEARCH_MIN_LENGTH;

  useEffect(() => {
    if (!active) return;
    const controller = new AbortController();
    const timer = setTimeout(() => {
      const request =
        slug === null
          ? api.search.posts(query, SEARCH_SUGGESTION_LIMIT, controller.signal)
          : api.search.blogPosts(slug, query, SEARCH_SUGGESTION_LIMIT, controller.signal);
      request
        .then((posts) => {
          if (!controller.signal.aborted) setSettled({ key, status: "done", posts });
        })
        .catch((err: unknown) => {
          if (controller.signal.aborted) return;
          const status = err instanceof ApiClientError && err.status === 429 ? "rate_limited" : "error";
          setSettled({ key, status, posts: [] });
        });
    }, SEARCH_DEBOUNCE_MS);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [active, slug, query, key]);

  const current = settled !== null && settled.key === key ? settled : null;

  if (!enabled || query.length === 0) return { status: "idle", suggestions: [] };
  if (query.length < SEARCH_MIN_LENGTH) return { status: "short", suggestions: [] };
  if (!current) return { status: "loading", suggestions: [] };
  const suggestions = current.posts.map((post) => ({ post, href: resultHref(scope, post) }));
  return { status: current.status, suggestions };
}
