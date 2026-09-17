"use client";

import { useLocale, useTranslations } from "next-intl";
import Link from "next/link";
import { useCallback, useEffect, useState, type FormEvent } from "react";

import { StatusPill } from "@/components/blog/StatusPill";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { FieldGroup, Input, Label, TextArea } from "@/components/ui/Field";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { useToast } from "@/components/ui/Toast";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import { displayPostStatus } from "@/lib/post-status";
import type { Chapter, Publication, PublicationDetail } from "@/lib/types";

import { errorMessage } from "./shared";

/** Gestione pubblicazioni (mockup 3g): elenco, creazione/modifica, indice dei
 * capitoli riordinabile (drag & drop e frecce), eliminazione. I post entrano
 * in una pubblicazione dal rail dell'editor (`PublicationSelect`). */
export function PublicationsTab({ blogSlug, canWrite }: { blogSlug: string; canWrite: boolean }) {
  const { accessToken, authFetch } = useAuth();
  const t = useTranslations("PublicationsTab");
  const tc = useTranslations("Common");
  const locale = useLocale();
  const notify = useToast();
  const [list, setList] = useState<Publication[] | null>(null);
  const [selected, setSelected] = useState<PublicationDetail | null>(null);
  const [chapters, setChapters] = useState<Chapter[]>([]);
  const [dragging, setDragging] = useState<string | null>(null);
  const [form, setForm] = useState<{ name: string; title: string; description: string } | null>(null);
  const [editing, setEditing] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    api.blogs.listPublications(blogSlug, accessToken).then(setList).catch((err) => setError(errorMessage(err)));
  }, [blogSlug, accessToken]);
  useEffect(load, [load]);

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

  async function open(p: Publication) {
    try {
      const detail = await api.blogs.getPublication(blogSlug, p.id, accessToken);
      setSelected(detail);
      setChapters(detail.chapters);
      setEditing(false);
      setForm(null);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  async function submit(e: FormEvent) {
    e.preventDefault();
    if (!form) return;
    if (editing && selected) {
      await act(async () => {
        await authFetch((token) => api.blogs.updatePublication(token, blogSlug, selected.id, { name: form.name, title: form.title, description: form.description }));
        setSelected(await api.blogs.getPublication(blogSlug, selected.id, accessToken));
        setEditing(false);
        setForm(null);
      }, t("saved"));
    } else {
      await act(async () => {
        const created = await authFetch((token) => api.blogs.createPublication(token, blogSlug, { name: form.name, title: form.title, description: form.description }));
        setForm(null);
        await open(created);
      }, t("created"));
    }
  }

  function move(from: number, to: number) {
    if (to < 0 || to >= chapters.length || from === to) return;
    const next = chapters.slice();
    const [item] = next.splice(from, 1);
    next.splice(to, 0, item);
    setChapters(next.map((c, i) => ({ ...c, n: i + 1 })));
  }

  const orderDirty = selected ? chapters.map((c) => c.post_id).join() !== selected.chapters.map((c) => c.post_id).join() : false;

  return (
    <div className={`grid gap-6 ${selected || form ? "xl:grid-cols-[minmax(0,1fr)_380px]" : ""}`}>
      <div className="flex flex-col gap-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <span className="text-sm text-muted">{t("intro")}</span>
          {canWrite && (
            <Button size="sm" onClick={() => { setSelected(null); setEditing(false); setForm({ name: "", title: "", description: "" }); }}>
              {t("new")}
            </Button>
          )}
        </div>
        {error && <Alert kind="error">{error}</Alert>}
        {list === null && !error && <SkeletonRows rows={3} />}
        {list !== null && list.length === 0 && <EmptyState glyph="▤" title={t("emptyTitle")} body={t("emptyBody")} />}
        {list && list.length > 0 && (
          <ul className="flex flex-col rounded-xl border border-border bg-surface">
            {list.map((p) => (
              <li key={p.id} className={`cursor-pointer border-b border-border px-4 py-3 last:border-0 hover:bg-background ${selected?.id === p.id ? "bg-background" : ""}`} onClick={() => open(p)}>
                <div className="flex flex-wrap items-baseline justify-between gap-2">
                  <span className="font-serif text-[17px] text-foreground">{p.title}</span>
                  <span className="text-[13px] text-muted">{t("chaptersOf", { published: p.chapters_published, total: p.chapters_total })}</span>
                </div>
                <span className="font-mono text-xs text-muted">/{blogSlug}/pub/{p.name}</span>
              </li>
            ))}
          </ul>
        )}

        {selected && (
          <Card className="flex flex-col gap-3">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <CardTitle>{t("chapters")}</CardTitle>
              {canWrite && orderDirty && (
                <Button size="sm" disabled={busy} onClick={() => act(async () => { const d = await authFetch((token) => api.blogs.orderPublication(token, blogSlug, selected.id, chapters.map((c) => c.post_id))); setSelected(d); setChapters(d.chapters); }, t("orderSaved"))}>
                  {t("saveOrder")}
                </Button>
              )}
            </div>
            {chapters.length === 0 ? (
              <p className="text-[13px] text-muted">{t("noChapters")}</p>
            ) : (
              <ol className="flex flex-col">
                {chapters.map((c, i) => (
                  <li
                    key={c.post_id}
                    draggable={canWrite}
                    onDragStart={() => setDragging(c.post_id)}
                    onDragOver={(e) => e.preventDefault()}
                    onDrop={() => {
                      if (dragging) move(chapters.findIndex((x) => x.post_id === dragging), i);
                      setDragging(null);
                    }}
                    className={`grid grid-cols-[32px_minmax(0,1fr)_auto] items-center gap-3 border-b border-border py-2.5 last:border-0 ${dragging === c.post_id ? "opacity-50" : ""}`}
                  >
                    <span className="font-serif text-lg text-muted">{c.n}</span>
                    <div className="flex min-w-0 flex-col">
                      <Link href={`/dashboard/blogs/${blogSlug}/posts/${c.post_id}`} className="truncate text-foreground no-underline hover:underline">
                        {c.title}
                      </Link>
                      <span className="text-xs text-muted">
                        {c.locale.toUpperCase()} · {t("minutes", { min: c.reading_minutes })}
                        {c.published_at && ` · ${formatDate(c.published_at, locale, { day: "numeric", month: "short", year: "numeric" })}`}
                      </span>
                    </div>
                    <div className="flex items-center gap-2">
                      <StatusPill status={displayPostStatus({ status: c.status, published_at: c.published_at })} />
                      {canWrite && (
                        <span className="flex flex-col text-xs text-muted">
                          <button type="button" onClick={() => move(i, i - 1)} disabled={i === 0} aria-label={t("moveUp")} className="hover:text-foreground disabled:opacity-30">▲</button>
                          <button type="button" onClick={() => move(i, i + 1)} disabled={i === chapters.length - 1} aria-label={t("moveDown")} className="hover:text-foreground disabled:opacity-30">▼</button>
                        </span>
                      )}
                    </div>
                  </li>
                ))}
              </ol>
            )}
            <span className="text-[13px] text-muted">{t("orderHint")}</span>
          </Card>
        )}
      </div>

      {(form || selected) && (
        <aside className="flex h-fit flex-col gap-4 rounded-xl border border-border bg-surface p-5 xl:sticky xl:top-6">
          {form ? (
            <form onSubmit={submit} className="flex flex-col gap-3">
              <span className="font-serif text-lg text-foreground">{editing ? t("edit") : t("new")}</span>
              <FieldGroup className="mb-0">
                <Label htmlFor="pub-title">{t("title")}</Label>
                <Input id="pub-title" required value={form.title} onChange={(e) => setForm({ ...form, title: e.target.value })} />
              </FieldGroup>
              <FieldGroup className="mb-0">
                <Label htmlFor="pub-name" hint={t("nameHint")}>
                  {t("name")}
                </Label>
                <Input id="pub-name" required pattern="[a-z0-9]+(-[a-z0-9]+)*" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value.toLowerCase() })} />
              </FieldGroup>
              <FieldGroup className="mb-0">
                <Label htmlFor="pub-desc">{t("description")}</Label>
                <TextArea id="pub-desc" rows={3} value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} />
              </FieldGroup>
              <div className="flex gap-2">
                <Button size="sm" type="submit" disabled={busy}>
                  {editing ? tc("save") : tc("add")}
                </Button>
                <Button size="sm" variant="ghost" type="button" onClick={() => { setForm(null); setEditing(false); }}>
                  {tc("cancel")}
                </Button>
              </div>
            </form>
          ) : selected ? (
            <>
              <div className="flex items-start justify-between gap-3">
                <div className="flex flex-col gap-0.5">
                  <span className="font-serif text-lg text-foreground">{selected.title}</span>
                  <span className="font-mono text-xs text-muted">/{blogSlug}/pub/{selected.name}</span>
                </div>
                <button type="button" onClick={() => setSelected(null)} className="text-muted hover:text-foreground" aria-label={tc("close")}>
                  ✕
                </button>
              </div>
              {selected.description && <p className="text-[13px] text-muted">{selected.description}</p>}
              <span className="text-[13px] text-muted">{t("chaptersOf", { published: selected.chapters_published, total: selected.chapters_total })}</span>
              <div className="flex flex-wrap gap-2">
                {selected.chapters_published > 0 && (
                  <Link href={`/${blogSlug}/pub/${selected.name}`} className="no-underline">
                    <Button size="sm" variant="secondary">
                      {t("view")} ↗
                    </Button>
                  </Link>
                )}
                {canWrite && (
                  <>
                    <Button size="sm" variant="secondary" onClick={() => { setEditing(true); setForm({ name: selected.name, title: selected.title, description: selected.description ?? "" }); }}>
                      {tc("edit")}
                    </Button>
                    <Button size="sm" variant="ghost" onClick={() => setConfirmDelete(true)}>
                      {tc("delete")}
                    </Button>
                  </>
                )}
              </div>
              <span className="text-[13px] text-muted">{t("addHint")}</span>
              <ConfirmDialog
                open={confirmDelete}
                title={t("deleteTitle", { title: selected.title })}
                body={t("deleteBody")}
                confirmText={selected.name}
                confirmLabel={tc("delete")}
                onCancel={() => setConfirmDelete(false)}
                onConfirm={() => { setConfirmDelete(false); void act(async () => { await authFetch((token) => api.blogs.deletePublication(token, blogSlug, selected.id)); setSelected(null); }, t("deleted")); }}
              />
            </>
          ) : null}
        </aside>
      )}
    </div>
  );
}
