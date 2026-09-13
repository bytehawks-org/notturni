"use client";

import Link from "next/link";
import { useCallback, useEffect, useMemo, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { FragmentCollectionEntry } from "@/lib/types";

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString("it-IT", { day: "numeric", month: "long", year: "numeric" });
}

type GroupMode = "none" | "post" | "author";

const GROUP_OPTIONS: { value: GroupMode; label: string }[] = [
  { value: "none", label: "Tutti" },
  { value: "post", label: "Per post" },
  { value: "author", label: "Per autore" },
];

function groupFragments(
  fragments: FragmentCollectionEntry[],
  mode: GroupMode
): { key: string; label: string; items: FragmentCollectionEntry[] }[] {
  if (mode === "none") return [{ key: "all", label: "", items: fragments }];
  const keyOf = (f: FragmentCollectionEntry) => (mode === "post" ? f.post_title : f.author_display_name);
  const groups = new Map<string, FragmentCollectionEntry[]>();
  for (const fragment of fragments) {
    const key = keyOf(fragment);
    groups.set(key, [...(groups.get(key) ?? []), fragment]);
  }
  return [...groups.entries()].map(([label, items]) => ({ key: label, label, items }));
}

export default function FragmentsPage() {
  const { authFetch } = useAuth();
  const [fragments, setFragments] = useState<FragmentCollectionEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [groupMode, setGroupMode] = useState<GroupMode>("none");
  const [copiedId, setCopiedId] = useState<string | null>(null);

  const load = useCallback(() => {
    authFetch((token) => api.fragments.listMine(token))
      .then(setFragments)
      .catch((err) => setError(err instanceof ApiClientError ? err.message : "Errore imprevisto."));
  }, [authFetch]);

  useEffect(load, [load]);

  async function handleRemove(id: string) {
    try {
      await authFetch((token) => api.fragments.remove(token, id));
      setFragments((prev) => prev?.filter((f) => f.id !== id) ?? null);
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : "Impossibile rimuovere il frammento.");
    }
  }

  async function handleToggleVisibility(id: string, isPublic: boolean) {
    try {
      const updated = await authFetch((token) => api.fragments.setPublic(token, id, isPublic));
      setFragments(
        (prev) => prev?.map((f) => (f.id === id ? { ...f, is_public: updated.is_public } : f)) ?? null
      );
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : "Impossibile cambiare la visibilità.");
    }
  }

  async function handleCopy(fragment: FragmentCollectionEntry) {
    try {
      await navigator.clipboard.writeText(fragment.text);
      setCopiedId(fragment.id);
      setTimeout(() => setCopiedId((current) => (current === fragment.id ? null : current)), 1500);
    } catch {
      // clipboard non disponibile (permessi/contesto non sicuro): nessun blocco, solo niente feedback
    }
  }

  const authorCount = useMemo(
    () => (fragments ? new Set(fragments.map((f) => f.author_display_name)).size : 0),
    [fragments]
  );
  const groups = useMemo(() => groupFragments(fragments ?? [], groupMode), [fragments, groupMode]);

  return (
    <div className="mx-auto max-w-2xl">
      <div className="mb-5 flex flex-wrap items-end justify-between gap-3">
        <div className="flex flex-col gap-0.5">
          <h1 className="font-serif text-2xl font-medium text-foreground">Frammenti</h1>
          {fragments && fragments.length > 0 && (
            <span className="text-[13px] text-muted">
              {fragments.length} salvati · {authorCount} autori · privati a te, salvo quelli resi pubblici
            </span>
          )}
        </div>
      </div>

      {fragments && fragments.length > 0 && (
        <div className="mb-5 flex gap-2 overflow-x-auto text-[13px]">
          {GROUP_OPTIONS.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() => setGroupMode(opt.value)}
              className={`shrink-0 rounded-full px-3 py-1.5 whitespace-nowrap ${
                groupMode === opt.value
                  ? "bg-foreground text-background"
                  : "border border-border text-muted hover:text-foreground"
              }`}
            >
              {opt.label}
            </button>
          ))}
        </div>
      )}

      {error && (
        <div className="mb-5">
          <Alert kind="error">{error}</Alert>
        </div>
      )}

      {fragments === null ? (
        <p className="text-sm text-muted">Caricamento…</p>
      ) : fragments.length === 0 ? (
        <Card>
          <p className="text-sm text-muted">
            Nessun frammento salvato. Seleziona una porzione di testo in un post (fino al 15% del testo)
            per evidenziarla e salvarla qui.
          </p>
        </Card>
      ) : (
        <div className="flex flex-col gap-8">
          {groups.map((group) => (
            <div key={group.key} className="flex flex-col gap-3.5">
              {group.label && (
                <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{group.label}</span>
              )}
              <ul className="flex flex-col gap-3.5">
                {group.items.map((fragment) => (
                  <li key={fragment.id}>
                    <Card className="flex flex-col gap-2.5">
                      <Link href={fragment.permalink}>
                        <blockquote className="fragment-quote font-serif text-base leading-relaxed text-foreground">
                          “{fragment.text}”
                        </blockquote>
                      </Link>
                      <div className="flex items-center justify-between gap-3 text-xs text-muted">
                        <span className="truncate">
                          <span className="font-semibold text-foreground">{fragment.author_display_name}</span> ·{" "}
                          {fragment.post_title}
                        </span>
                        <span className="shrink-0">{formatDate(fragment.created_at)}</span>
                      </div>
                      <div className="flex items-center gap-4 text-[13px]">
                        <Link href={fragment.permalink} className="font-medium text-primary hover:underline">
                          Apri nel post
                        </Link>
                        <button type="button" onClick={() => handleCopy(fragment)} className="text-muted hover:text-foreground">
                          {copiedId === fragment.id ? "Copiato" : "Copia"}
                        </button>
                        <button
                          type="button"
                          onClick={() => handleToggleVisibility(fragment.id, !fragment.is_public)}
                          className={fragment.is_public ? "text-primary hover:underline" : "text-muted hover:text-foreground"}
                          title="Cambia visibilità"
                        >
                          {fragment.is_public ? "Pubblico" : "Privato"}
                        </button>
                        <button
                          type="button"
                          onClick={() => handleRemove(fragment.id)}
                          className="ml-auto text-muted hover:text-foreground"
                        >
                          Rimuovi
                        </button>
                      </div>
                    </Card>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
