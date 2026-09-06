"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Card } from "@/components/ui/Card";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { FragmentCollectionEntry } from "@/lib/types";

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString("it-IT", { day: "numeric", month: "long", year: "numeric" });
}

export default function FragmentsPage() {
  const { authFetch } = useAuth();
  const [fragments, setFragments] = useState<FragmentCollectionEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

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

  return (
    <div className="space-y-6">
      <div>
        <h1 className="font-serif text-2xl text-foreground">Frammenti</h1>
        <p className="mt-1 text-sm text-muted">
          Porzioni di testo evidenziate durante la lettura: salvate qui, e ri-evidenziate ogni volta che
          torni sul post originale. Ogni frammento è <strong>privato</strong> (visibile solo a te) o{" "}
          <strong>pubblico</strong> (visibile ad altri utenti iscritti alla piattaforma, mai a chi non ha
          un account) — puoi cambiarlo in qualsiasi momento con il pulsante accanto a &quot;Rimuovi&quot;.
        </p>
      </div>

      {error && <Alert kind="error">{error}</Alert>}

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
        <ul className="space-y-4">
          {fragments.map((fragment) => (
            <li key={fragment.id}>
              <Card className="relative">
                <Link href={fragment.permalink} className="block pr-16">
                  <blockquote className="fragment-quote font-serif text-lg leading-relaxed text-foreground">
                    “{fragment.text}”
                  </blockquote>
                  <p className="mt-3 text-sm text-muted">
                    {fragment.post_title} · {fragment.author_display_name} · {formatDate(fragment.created_at)}
                  </p>
                </Link>
                <div className="absolute right-6 top-6 flex items-center gap-3 text-sm">
                  <button
                    type="button"
                    onClick={() => handleToggleVisibility(fragment.id, !fragment.is_public)}
                    className={fragment.is_public ? "text-primary hover:underline" : "text-muted hover:text-foreground"}
                    title="Cambia visibilità"
                  >
                    {fragment.is_public ? "Pubblico" : "Privato"}
                  </button>
                  <button type="button" onClick={() => handleRemove(fragment.id)} className="text-muted hover:text-foreground">
                    Rimuovi
                  </button>
                </div>
              </Card>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
