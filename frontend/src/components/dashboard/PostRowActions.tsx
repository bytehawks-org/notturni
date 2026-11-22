"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useState } from "react";

import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { DisplayPostStatus } from "@/lib/post-status";
import type { Post } from "@/lib/types";

const ACTION = "text-[13px] text-muted no-underline hover:text-foreground disabled:opacity-50";

/** Azioni di riga di un post nell'elenco della dashboard: vedi (se pubblico), modifica,
 * rimuovi dalla pubblicazione (se pubblicato/pianificato) ed elimina (con conferma digitando lo slug).
 * I permessi sono quelli del backend: senza scrittura sul blog la richiesta torna 403 e il messaggio
 * compare sotto la riga. */
export function PostRowActions({
  post,
  status,
  onUpdated,
  onDeleted,
}: {
  post: Post;
  status: DisplayPostStatus;
  onUpdated: (post: Post) => void;
  onDeleted: (postId: string) => void;
}) {
  const t = useTranslations("PostRowActions");
  const tc = useTranslations("Common");
  const { authFetch } = useAuth();
  const [busy, setBusy] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async (fn: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : tc("unexpectedError"));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex flex-col items-end gap-1" aria-label={t("actions")}>
      <div className="flex flex-wrap items-center justify-end gap-x-3 gap-y-1">
        {status === "published" && (
          <Link href={post.permalink} className={ACTION}>
            {t("view")} ↗
          </Link>
        )}
        <Link href={`/dashboard/blogs/${post.blog_slug}/posts/${post.id}`} className={ACTION}>
          {t("edit")}
        </Link>
        {(status === "published" || status === "scheduled") && (
          <button
            type="button"
            className={ACTION}
            disabled={busy}
            onClick={() => run(async () => onUpdated(await authFetch((token) => api.posts.unpublish(token, post.id))))}
          >
            {t("unpublish")}
          </button>
        )}
        <button
          type="button"
          className="text-[13px] text-danger hover:underline disabled:opacity-50"
          disabled={busy}
          onClick={() => setConfirming(true)}
        >
          {t("delete")}
        </button>
      </div>
      {error && <span className="text-[12px] text-danger">{error}</span>}
      <ConfirmDialog
        open={confirming}
        title={t("deleteTitle")}
        body={t("deleteBody")}
        confirmText={post.slug}
        confirmLabel={t("deleteConfirm")}
        onCancel={() => setConfirming(false)}
        onConfirm={() => {
          setConfirming(false);
          void run(async () => {
            await authFetch((token) => api.posts.remove(token, post.id));
            onDeleted(post.id);
          });
        }}
      />
    </div>
  );
}
