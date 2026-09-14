"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useCallback, useEffect, useRef, useState } from "react";

import { FragmentMenu, type FragmentSelection } from "@/components/blog/FragmentMenu";
import { Alert } from "@/components/ui/Alert";
import { useToast } from "@/components/ui/Toast";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import {
  findOverlappingFragmentIds,
  highlightFragments,
  normalizeFragmentText,
  unwrapFragmentMark,
} from "@/lib/highlight-fragments";
import { type PostFragment } from "@/lib/types";

type SelectionMenu = { sel: FragmentSelection; fragmentIds: string[] };

/** Contenuto reso del post (HTML già sanificato lato server, vedi
 * lib/markdown.ts) con evidenziazione e salvataggio dei frammenti: selezione
 * → menu contestuale (mockup 1a/1b) → salvataggio → raccolta unificata
 * (/dashboard/frammenti). I frammenti già salvati dall'utente su questo post
 * vengono ri-evidenziati ad ogni lettura. "Copia link"/"Cita" funzionano
 * anche da anonimi: sono pure operazioni sugli appunti. */
export function FragmentReader({
  postId,
  html,
  className,
  permalink,
  quoteAttribution,
}: {
  postId: string;
  html: string;
  className: string;
  /** URL assoluto o relativo del post, per "Copia link"/"Cita". */
  permalink: string;
  /** "Autore, Titolo" da accodare alla citazione. */
  quoteAttribution: string;
}) {
  const { user, loading, authFetch } = useAuth();
  const t = useTranslations("Fragment");
  const notify = useToast();
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

  const handleSelectionEnd = useCallback(() => {
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
    const text = normalizeFragmentText(selection.toString());
    if (!text) {
      setMenu(null);
      return;
    }
    const totalLength = Math.max(1, normalizeFragmentText(container.textContent ?? "").length);
    const sel: FragmentSelection = {
      text,
      words: text.split(/\s+/).filter(Boolean).length,
      ratio: text.length / totalLength,
      top: rect.top,
      left: rect.left + rect.width / 2,
    };
    // ri-selezionare (anche solo in parte) un frammento già evidenziato
    // propone di rimuoverlo, invece di salvarne uno nuovo.
    setMenu({ sel, fragmentIds: findOverlappingFragmentIds(container, range) });
  }, []);

  useEffect(() => {
    document.addEventListener("mouseup", handleSelectionEnd);
    document.addEventListener("touchend", handleSelectionEnd);
    // lo scroll non fa scattare mouseup: il menu resterebbe ancorato a una
    // posizione (position: fixed) non più sotto la selezione.
    const closeOnScroll = () => setMenu((m) => (m && window.innerWidth >= 768 ? null : m));
    window.addEventListener("scroll", closeOnScroll, true);
    return () => {
      document.removeEventListener("mouseup", handleSelectionEnd);
      document.removeEventListener("touchend", handleSelectionEnd);
      window.removeEventListener("scroll", closeOnScroll, true);
    };
  }, [handleSelectionEnd]);

  function close() {
    window.getSelection()?.removeAllRanges();
    setMenu(null);
  }

  async function copy(text: string, doneMessage: string) {
    try {
      await navigator.clipboard.writeText(text);
      notify(doneMessage);
    } catch {
      notify(t("copyFailed"), "warn");
    }
    close();
  }

  function absolutePermalink(): string {
    return permalink.startsWith("http") ? permalink : `${window.location.origin}${permalink}`;
  }

  async function handleSave(isPublic: boolean) {
    if (!menu || menu.sel.ratio > 0.15) return;
    setSaving(true);
    setError(null);
    try {
      const fragment = await authFetch((token) => api.fragments.create(token, postId, menu.sel.text, isPublic));
      setFragments((prev) => (prev.some((f) => f.id === fragment.id) ? prev : [...prev, fragment]));
      notify(t("savedToast"));
      close();
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : t("saveError"));
    } finally {
      setSaving(false);
    }
  }

  async function handleRemove() {
    if (!menu || menu.fragmentIds.length === 0) return;
    setSaving(true);
    setError(null);
    try {
      await Promise.all(menu.fragmentIds.map((id) => authFetch((token) => api.fragments.remove(token, id))));
      if (containerRef.current) {
        for (const id of menu.fragmentIds) unwrapFragmentMark(containerRef.current, id);
      }
      setFragments((prev) => prev.filter((f) => !menu.fragmentIds.includes(f.id)));
      close();
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : t("removeError"));
    } finally {
      setSaving(false);
    }
  }

  return (
    <>
      <div ref={containerRef} className={className} dangerouslySetInnerHTML={{ __html: html }} />
      {menu && (
        <FragmentMenu
          sel={menu.sel}
          saved={menu.fragmentIds.length > 0}
          signedIn={!loading && !!user}
          busy={saving}
          onSave={handleSave}
          onRemove={handleRemove}
          onCopyLink={() => copy(absolutePermalink(), t("linkCopied"))}
          onQuote={() => copy(`“${menu.sel.text}”\n— ${quoteAttribution}\n${absolutePermalink()}`, t("quoteCopied"))}
          onClose={close}
        />
      )}
      {error && (
        <div className="mt-4">
          <Alert kind="error">{error}</Alert>
        </div>
      )}
      {user && fragments.length > 0 && (
        <aside className="mt-8 flex flex-wrap items-center justify-between gap-2 rounded-xl border border-border bg-surface px-4 py-3 text-[13px] text-muted">
          <span>
            <span className="font-semibold text-foreground">{t("yourFragments")}</span>{" "}
            {t("savedFromPost", { count: fragments.length })}
          </span>
          <Link href="/dashboard/frammenti" className="font-medium text-primary no-underline hover:underline">
            {t("openShelf")}
          </Link>
        </aside>
      )}
    </>
  );
}
