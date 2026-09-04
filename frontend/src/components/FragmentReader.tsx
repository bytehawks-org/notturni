"use client";

import Link from "next/link";
import { useCallback, useEffect, useRef, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import {
  findOverlappingFragmentIds,
  highlightFragments,
  normalizeFragmentText,
  unwrapFragmentMark,
} from "@/lib/highlight-fragments";
import { MAX_FRAGMENT_RATIO, type PostFragment } from "@/lib/types";

type SelectionMenu =
  | { mode: "save"; top: number; left: number; text: string; tooLong: boolean }
  | { mode: "remove"; top: number; left: number; fragmentIds: string[] };

/** Contenuto reso del post (HTML già sanificato lato server, vedi
 * lib/markdown.ts) con evidenziazione e salvataggio dei frammenti: selezione
 * con il mouse → menu contestuale → salvataggio → raccolta unificata
 * (/dashboard/frammenti). I frammenti già salvati dall'utente su questo post
 * vengono ri-evidenziati ad ogni lettura, non solo entrando dalla pagina di
 * raccolta. */
export function FragmentReader({
  postId,
  html,
  className,
}: {
  postId: string;
  html: string;
  className: string;
}) {
  const { user, loading, authFetch } = useAuth();
  const containerRef = useRef<HTMLDivElement>(null);
  const [fragments, setFragments] = useState<PostFragment[]>([]);
  const [menu, setMenu] = useState<SelectionMenu | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (loading || !user) return;
    authFetch((token) => api.fragments.listForPost(token, postId))
      .then(setFragments)
      .catch(() => undefined);
  }, [loading, user, postId, authFetch]);

  useEffect(() => {
    if (containerRef.current) highlightFragments(containerRef.current, fragments);
  }, [fragments, html]);

  const handleMouseUp = useCallback(() => {
    const container = containerRef.current;
    const selection = window.getSelection();
    if (!container || !selection || selection.isCollapsed || selection.rangeCount === 0) {
      setMenu(null);
      return;
    }
    const range = selection.getRangeAt(0);
    if (!container.contains(range.commonAncestorContainer)) {
      setMenu(null);
      return;
    }
    const rect = range.getBoundingClientRect();
    setError(null);

    // ri-selezionare (anche solo in parte) un frammento già evidenziato
    // propone di rimuoverlo, invece di salvarne uno nuovo.
    const overlapping = findOverlappingFragmentIds(container, range);
    if (overlapping.length > 0) {
      setMenu({ mode: "remove", top: rect.top, left: rect.left + rect.width / 2, fragmentIds: overlapping });
      return;
    }

    const text = normalizeFragmentText(selection.toString());
    if (!text) {
      setMenu(null);
      return;
    }
    const totalLength = normalizeFragmentText(container.textContent ?? "").length;
    const maxLength = Math.max(1, Math.floor(totalLength * MAX_FRAGMENT_RATIO));
    setMenu({
      mode: "save",
      top: rect.top,
      left: rect.left + rect.width / 2,
      text,
      tooLong: text.length > maxLength,
    });
  }, []);

  useEffect(() => {
    document.addEventListener("mouseup", handleMouseUp);
    // lo scroll non fa scattare mouseup: il menu resterebbe ancorato a una
    // posizione (position: fixed) non più sotto la selezione.
    const closeOnScroll = () => setMenu(null);
    window.addEventListener("scroll", closeOnScroll, true);
    return () => {
      document.removeEventListener("mouseup", handleMouseUp);
      window.removeEventListener("scroll", closeOnScroll, true);
    };
  }, [handleMouseUp]);

  async function handleSave() {
    if (!menu || menu.mode !== "save" || menu.tooLong) return;
    setSaving(true);
    setError(null);
    try {
      const fragment = await authFetch((token) => api.fragments.create(token, postId, menu.text));
      setFragments((prev) => (prev.some((f) => f.id === fragment.id) ? prev : [...prev, fragment]));
      window.getSelection()?.removeAllRanges();
      setMenu(null);
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : "Impossibile salvare il frammento.");
    } finally {
      setSaving(false);
    }
  }

  async function handleRemove() {
    if (!menu || menu.mode !== "remove") return;
    setSaving(true);
    setError(null);
    try {
      await Promise.all(menu.fragmentIds.map((id) => authFetch((token) => api.fragments.remove(token, id))));
      if (containerRef.current) {
        for (const id of menu.fragmentIds) unwrapFragmentMark(containerRef.current, id);
      }
      setFragments((prev) => prev.filter((f) => !menu.fragmentIds.includes(f.id)));
      window.getSelection()?.removeAllRanges();
      setMenu(null);
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : "Impossibile rimuovere il frammento.");
    } finally {
      setSaving(false);
    }
  }

  return (
    <>
      <div ref={containerRef} className={className} dangerouslySetInnerHTML={{ __html: html }} />
      {menu && (
        <div
          className="fragment-menu"
          style={{ top: menu.top, left: menu.left }}
          // impedisce che il mousedown sul menu tolga il focus dal testo e
          // collassi la selezione prima che il click arrivi al bottone
          onMouseDown={(e) => e.preventDefault()}
        >
          {loading ? null : !user ? (
            <Link href="/login" className="fragment-menu-link">
              Accedi per salvare i frammenti
            </Link>
          ) : menu.mode === "remove" ? (
            <button
              type="button"
              className="fragment-menu-button fragment-menu-button--remove"
              onClick={handleRemove}
              disabled={saving}
            >
              {saving ? "Rimuovo…" : menu.fragmentIds.length > 1 ? "Rimuovi frammenti" : "Rimuovi frammento"}
            </button>
          ) : menu.tooLong ? (
            <span className="fragment-menu-hint">Seleziona una porzione più breve (max 15% del post)</span>
          ) : (
            <button type="button" className="fragment-menu-button" onClick={handleSave} disabled={saving}>
              {saving ? "Salvo…" : "Salva frammento"}
            </button>
          )}
        </div>
      )}
      {error && (
        <div className="mt-4">
          <Alert kind="error">{error}</Alert>
        </div>
      )}
    </>
  );
}
