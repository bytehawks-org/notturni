"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

import { StatusPill } from "@/components/blog/StatusPill";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { useToast } from "@/components/ui/Toast";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { indent, moveDown, moveUp, outdent, toItems } from "@/lib/outline";
import type { DisplayPostStatus } from "@/lib/post-status";
import type { OutlineNode } from "@/lib/types";

import { errorMessage } from "./shared";

const MAX_DEPTH = 8;

function displayStatus(n: OutlineNode): DisplayPostStatus {
  if (n.status === "pending_review") return "review";
  if (n.status === "published") return n.is_visible ? "published" : "scheduled";
  return "draft";
}

/** Indice di uno Spazio wiki/docs: l'albero delle pagine con frecce per riordinare, indentare e
 * de-indentare. Le modifiche restano locali finché non si salva (un solo `PUT`, atomico). */
export function OutlineTab({ blogSlug, canWrite }: { blogSlug: string; canWrite: boolean }) {
  const { accessToken, authFetch } = useAuth();
  const t = useTranslations("OutlineTab");
  const notify = useToast();
  const [nodes, setNodes] = useState<OutlineNode[] | null>(null);
  const [saved, setSaved] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const signature = (list: OutlineNode[]) => JSON.stringify(toItems(list));

  const load = useCallback(() => {
    api.wiki
      .outline(accessToken, blogSlug)
      .then((list) => {
        setNodes(list);
        setSaved(signature(list));
      })
      .catch((err) => setError(errorMessage(err)));
  }, [blogSlug, accessToken]);
  useEffect(load, [load]);

  if (error && !nodes) return <Alert kind="error">{error}</Alert>;
  if (!nodes) return <SkeletonRows rows={4} />;
  if (nodes.length === 0) {
    return <EmptyState title={t("emptyTitle")} body={t("emptyBody")} />;
  }

  const dirty = signature(nodes) !== saved;
  const apply = (fn: (list: OutlineNode[]) => OutlineNode[]) => setNodes((list) => (list ? fn(list) : list));

  async function save() {
    if (!nodes) return;
    setBusy(true);
    setError(null);
    try {
      const updated = await authFetch((token) => api.wiki.saveOutline(token, blogSlug, toItems(nodes)));
      setNodes(updated);
      setSaved(signature(updated));
      notify(t("saved"));
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  const arrow = "rounded px-1.5 py-0.5 text-muted transition hover:bg-background hover:text-foreground disabled:opacity-30 disabled:hover:bg-transparent";

  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm text-muted">{t("hint")}</p>
      {error && <Alert kind="error">{error}</Alert>}
      <ul className="flex flex-col divide-y divide-border rounded-xl border border-border bg-surface">
        {nodes.map((n, i) => (
          <li key={n.id} className="flex items-center gap-3 px-3 py-2" style={{ paddingLeft: `${12 + n.depth * 24}px` }}>
            <Link
              href={`/dashboard/blogs/${blogSlug}/posts/${n.id}`}
              className="min-w-0 flex-1 truncate text-sm text-foreground no-underline hover:underline"
            >
              {n.title}
              <span className="ml-2 font-mono text-xs text-muted">
                {n.slug} · {n.locale}
              </span>
            </Link>
            <StatusPill status={displayStatus(n)} />
            {canWrite && (
              <span className="flex shrink-0 items-center">
                <button type="button" className={arrow} aria-label={t("up")} onClick={() => apply((l) => moveUp(l, i))}>
                  ↑
                </button>
                <button type="button" className={arrow} aria-label={t("down")} onClick={() => apply((l) => moveDown(l, i))}>
                  ↓
                </button>
                <button type="button" className={arrow} aria-label={t("outdent")} onClick={() => apply((l) => outdent(l, i))}>
                  ←
                </button>
                <button
                  type="button"
                  className={arrow}
                  aria-label={t("indent")}
                  onClick={() => apply((l) => indent(l, i, MAX_DEPTH))}
                >
                  →
                </button>
              </span>
            )}
          </li>
        ))}
      </ul>
      {canWrite && (
        <div className="flex items-center gap-3">
          <Button onClick={save} disabled={!dirty || busy}>
            {busy ? t("saving") : t("save")}
          </Button>
          {dirty && (
            <Button variant="secondary" onClick={load} disabled={busy}>
              {t("discard")}
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
