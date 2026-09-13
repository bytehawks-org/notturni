"use client";

import { useLocale, useTranslations } from "next-intl";
import Link from "next/link";
import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { FilterChip, Pill } from "@/components/ui/Pill";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { useToast } from "@/components/ui/Toast";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import type { BlogNote, NoteKind } from "@/lib/types";

import { errorMessage } from "./shared";

const KINDS: NoteKind[] = ["book", "article", "web", "note"];
type Filter = "all" | "unused" | "duplicates";

/** Libreria note del blog (mockup 3b): tabella con tipo/usi/aggiornamento,
 * ricerca e filtri, pannello con tipo, URL/DOI, testo, "citata in", unione
 * dei duplicati, nuova nota, import/export BibTeX. */
export function NotesTab({ blogSlug, canWrite }: { blogSlug: string; canWrite: boolean }) {
  const { authFetch } = useAuth();
  const t = useTranslations("NotesTab");
  const tb = useTranslations("Bibliography");
  const tc = useTranslations("Common");
  const locale = useLocale();
  const notify = useToast();
  const [notes, setNotes] = useState<BlogNote[] | null>(null);
  const [q, setQ] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [selected, setSelected] = useState<BlogNote | null>(null);
  const [content, setContent] = useState("");
  const [kind, setKind] = useState<NoteKind>("note");
  const [url, setUrl] = useState("");
  const [creating, setCreating] = useState(false);
  const [importing, setImporting] = useState(false);
  const [bibtex, setBibtex] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    authFetch((token) => api.blogs.listNotes(token, blogSlug))
      .then(setNotes)
      .catch((err) => setError(errorMessage(err)));
  }, [authFetch, blogSlug]);
  useEffect(load, [load]);

  function open(n: BlogNote | null) {
    setSelected(n);
    setCreating(false);
    setContent(n?.content ?? "");
    setKind(n?.kind ?? "note");
    setUrl(n?.url ?? "");
  }

  async function act(fn: () => Promise<unknown>, done?: string) {
    setBusy(true);
    setError(null);
    try {
      await fn();
      if (done) notify(done);
      load();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (creating) {
      await act(async () => {
        const n = await authFetch((token) => api.blogs.createNote(token, blogSlug, { content, kind, url: url || null }));
        open(n);
      }, t("created"));
    } else if (selected) {
      await act(async () => {
        const n = await authFetch((token) => api.blogs.updateNote(token, blogSlug, selected.id, { content, kind, url: url || null }));
        setSelected(n);
      }, t("saved"));
    }
  }

  async function exportBib() {
    try {
      const blob = await authFetch(async (token) => {
        const res = await fetch(api.blogs.notesExportUrl(blogSlug), { headers: { Authorization: `Bearer ${token}` } });
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        return res.blob();
      });
      const href = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = href;
      a.download = `${blogSlug}.bib`;
      a.click();
      URL.revokeObjectURL(href);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  const all = notes ?? [];
  const unusedCount = all.filter((n) => n.used_in.length === 0).length;
  const dupCount = all.filter((n) => n.possible_duplicates.length > 0).length;
  const visible = all
    .filter((n) => (filter === "unused" ? n.used_in.length === 0 : filter === "duplicates" ? n.possible_duplicates.length > 0 : true))
    .filter((n) => !q.trim() || n.content.toLowerCase().includes(q.trim().toLowerCase()));
  const byId = new Map(all.map((n) => [n.id, n]));
  const panelOpen = creating || selected !== null;

  return (
    <div className={`grid gap-6 ${panelOpen ? "xl:grid-cols-[minmax(0,1fr)_360px]" : ""}`}>
      <div className="flex flex-col gap-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder={t("search")}
            className="w-full max-w-xs rounded-lg border border-border bg-surface px-3 py-2 text-sm text-foreground focus:outline-none focus:border-primary"
          />
          <div className="flex items-center gap-2">
            <Button size="sm" variant="secondary" onClick={exportBib} disabled={all.length === 0}>
              {t("exportBibtex")}
            </Button>
            {canWrite && (
              <>
                <Button size="sm" variant="secondary" onClick={() => { setImporting((v) => !v); setSelected(null); setCreating(false); }}>
                  {t("importBibtex")}
                </Button>
                <Button size="sm" onClick={() => { setSelected(null); setCreating(true); setContent(""); setKind("note"); setUrl(""); setImporting(false); }}>
                  {t("newNote")}
                </Button>
              </>
            )}
          </div>
        </div>
        <div className="flex flex-wrap gap-1.5">
          <FilterChip active={filter === "all"} onClick={() => setFilter("all")}>
            {t("filter.all")} · {all.length}
          </FilterChip>
          <FilterChip active={filter === "unused"} onClick={() => setFilter("unused")}>
            {t("filter.unused")} · {unusedCount}
          </FilterChip>
          <FilterChip active={filter === "duplicates"} onClick={() => setFilter("duplicates")}>
            {t("filter.duplicates")} · {dupCount}
          </FilterChip>
        </div>
        {error && <Alert kind="error">{error}</Alert>}
        {importing && (
          <form
            className="flex flex-col gap-2 rounded-xl border border-border bg-surface p-4"
            onSubmit={(e) => {
              e.preventDefault();
              void act(async () => {
                const created = await authFetch((token) => api.blogs.importNotes(token, blogSlug, bibtex));
                notify(t("imported", { count: created.length }));
                setBibtex("");
                setImporting(false);
              });
            }}
          >
            <span className="text-[13px] text-muted">{t("importHint")}</span>
            <textarea required rows={6} value={bibtex} onChange={(e) => setBibtex(e.target.value)} className="rounded-lg border border-border bg-background px-3 py-2 font-mono text-xs text-foreground focus:outline-none focus:border-primary" placeholder="@book{...}" />
            <div className="flex gap-2">
              <Button size="sm" type="submit" disabled={busy}>
                {t("importBibtex")}
              </Button>
              <Button size="sm" variant="ghost" type="button" onClick={() => setImporting(false)}>
                {tc("cancel")}
              </Button>
            </div>
          </form>
        )}
        {notes === null && !error && <SkeletonRows rows={5} />}
        {notes !== null && visible.length === 0 && <EmptyState glyph="❞" title={t("emptyTitle")} body={t("emptyBody")} />}
        {visible.length > 0 && (
          <div className="overflow-x-auto rounded-xl border border-border bg-surface">
            <table className="w-full text-left text-sm">
              <thead className="border-b border-border font-mono text-[11px] uppercase tracking-[.06em] text-muted">
                <tr>
                  <th className="px-4 py-3">{t("col.note")}</th>
                  <th className="px-4 py-3">{t("col.kind")}</th>
                  <th className="px-4 py-3">{t("col.used")}</th>
                  <th className="px-4 py-3">{t("col.updated")}</th>
                </tr>
              </thead>
              <tbody>
                {visible.map((n) => (
                  <tr key={n.id} className={`cursor-pointer border-b border-border last:border-0 hover:bg-background ${selected?.id === n.id ? "bg-background" : ""}`} onClick={() => open(n)}>
                    <td className="px-4 py-3">
                      <span className="line-clamp-2 text-foreground">{n.content}</span>
                      {n.possible_duplicates.length > 0 && <Pill tone="warn">{t("duplicate")}</Pill>}
                    </td>
                    <td className="px-4 py-3">
                      <span className="rounded border border-border px-1.5 py-0.5 font-mono text-[11px] uppercase tracking-[.06em] text-muted">{tb(`kind.${n.kind}`)}</span>
                    </td>
                    <td className="px-4 py-3 text-muted">{n.used_in.length}</td>
                    <td className="whitespace-nowrap px-4 py-3 text-muted">{formatDate(n.updated_at, locale, { day: "numeric", month: "short" })}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {panelOpen && (
        <aside className="flex h-fit flex-col gap-4 rounded-xl border border-border bg-surface p-5 xl:sticky xl:top-6">
          <div className="flex items-start justify-between gap-3">
            <span className="font-serif text-lg text-foreground">{creating ? t("newNote") : t("note")}</span>
            <button type="button" onClick={() => open(null)} className="text-muted hover:text-foreground" aria-label={tc("close")}>
              ✕
            </button>
          </div>
          <form onSubmit={submit} className="flex flex-col gap-3">
            <label className="flex flex-col gap-1.5 text-sm">
              <span className="text-xs font-semibold uppercase tracking-[.04em] text-muted">{t("text")}</span>
              <textarea required rows={4} value={content} onChange={(e) => setContent(e.target.value)} disabled={!canWrite} className="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground focus:outline-none focus:border-primary" />
            </label>
            <div className="flex flex-col gap-1.5">
              <span className="text-xs font-semibold uppercase tracking-[.04em] text-muted">{t("kind")}</span>
              <div className="flex flex-wrap gap-1.5">
                {KINDS.map((k) => (
                  <FilterChip key={k} active={kind === k} onClick={() => canWrite && setKind(k)}>
                    {tb(`kind.${k}`)}
                  </FilterChip>
                ))}
              </div>
            </div>
            <label className="flex flex-col gap-1.5 text-sm">
              <span className="text-xs font-semibold uppercase tracking-[.04em] text-muted">{t("url")}</span>
              <input value={url} onChange={(e) => setUrl(e.target.value)} disabled={!canWrite} placeholder="https://doi.org/…" className="rounded-lg border border-border bg-background px-3 py-2 text-sm text-foreground focus:outline-none focus:border-primary" />
            </label>
            {canWrite && (
              <div className="flex flex-wrap gap-2">
                <Button size="sm" type="submit" disabled={busy || !content.trim()}>
                  {creating ? tc("add") : tc("save")}
                </Button>
                {selected && (
                  <Button size="sm" variant="ghost" type="button" disabled={busy || selected.used_in.length > 0} onClick={() => act(async () => { await authFetch((token) => api.blogs.deleteNote(token, blogSlug, selected.id)); open(null); }, t("deleted"))}>
                    {tc("delete")}
                  </Button>
                )}
              </div>
            )}
          </form>
          {selected && (
            <>
              <div className="flex flex-col gap-1 text-[13px]">
                <span className="text-xs font-semibold uppercase tracking-[.04em] text-muted">
                  {tb("citedIn")} · {selected.used_in.length}
                </span>
                {selected.used_in.length === 0 ? (
                  <span className="text-muted">{t("notUsed")}</span>
                ) : (
                  selected.used_in.map((u) => (
                    <Link key={`${u.post_id}-${u.idx}`} href={`/dashboard/blogs/${blogSlug}/posts/${u.post_id}`} className="text-primary no-underline hover:underline">
                      {u.post_title} <span className="text-muted">· {t("noteN", { n: u.idx })}</span>
                    </Link>
                  ))
                )}
              </div>
              {selected.possible_duplicates.length > 0 && (
                <div className="flex flex-col gap-2 rounded-lg bg-[var(--warn-bg)] p-3 text-[13px]">
                  <span className="font-semibold text-foreground">{t("possibleDuplicate")}</span>
                  {selected.possible_duplicates.map((id) => {
                    const other = byId.get(id);
                    if (!other) return null;
                    return (
                      <div key={id} className="flex flex-col gap-1">
                        <span className="line-clamp-2 text-muted">“{other.content}”</span>
                        {canWrite && (
                          <div className="flex gap-3">
                            <button type="button" className="text-primary hover:underline" onClick={() => act(async () => { const n = await authFetch((token) => api.blogs.mergeNote(token, blogSlug, other.id, selected.id)); setSelected(n); }, t("merged"))}>
                              {t("mergeInto", { which: t("this") })}
                            </button>
                            <button type="button" className="text-primary hover:underline" onClick={() => act(async () => { const n = await authFetch((token) => api.blogs.mergeNote(token, blogSlug, selected.id, other.id)); open(n); }, t("merged"))}>
                              {t("mergeInto", { which: t("other") })}
                            </button>
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </>
          )}
        </aside>
      )}
    </div>
  );
}
