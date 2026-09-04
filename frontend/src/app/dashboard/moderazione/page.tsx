"use client";

import { useCallback, useEffect, useState } from "react";

import { SearchInput } from "@/components/SearchInput";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { ADMIN_POST_STATUS_LABELS, type AdminPost } from "@/lib/types";

function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

export default function DashboardModerationPage() {
  const { authFetch } = useAuth();
  const [q, setQ] = useState("");
  const [posts, setPosts] = useState<AdminPost[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [rowError, setRowError] = useState<Record<string, string>>({});

  const load = useCallback(() => {
    authFetch((token) => api.admin.listPosts(token, q))
      .then(setPosts)
      .catch((err) => setError(errorMessage(err)));
  }, [authFetch, q]);

  useEffect(load, [load]);

  async function handleToggleHidden(postId: string, is_hidden: boolean) {
    setRowError((prev) => ({ ...prev, [postId]: "" }));
    try {
      const updated = await authFetch((token) => api.admin.updatePost(token, postId, { is_hidden }));
      setPosts((prev) => prev?.map((p) => (p.id === postId ? updated : p)) ?? null);
    } catch (err) {
      setRowError((prev) => ({ ...prev, [postId]: errorMessage(err) }));
    }
  }

  return (
    <div>
      <div className="mb-6 flex items-center justify-between">
        <h1 className="font-serif text-2xl text-foreground">Moderazione post</h1>
        <SearchInput value={q} onChange={setQ} placeholder="Cerca per titolo, slug, blog o autore…" />
      </div>
      {error && <Alert kind="error">{error}</Alert>}

      <div className="overflow-x-auto rounded-lg border border-border">
        <table className="w-full text-left text-sm">
          <thead className="border-b border-border text-muted">
            <tr>
              <th className="px-4 py-3">Titolo</th>
              <th className="px-4 py-3">Blog</th>
              <th className="px-4 py-3">Autore</th>
              <th className="px-4 py-3">Stato</th>
              <th className="px-4 py-3">Moderazione</th>
            </tr>
          </thead>
          <tbody>
            {posts?.map((p) => (
              <tr key={p.id} className="border-b border-border last:border-0">
                <td className="px-4 py-3 text-foreground">{p.title}</td>
                <td className="px-4 py-3 text-muted">{p.blog_slug}</td>
                <td className="px-4 py-3 text-muted">{p.author_username}</td>
                <td className="px-4 py-3 text-muted">{ADMIN_POST_STATUS_LABELS[p.status]}</td>
                <td className="px-4 py-3">
                  <Button
                    variant={p.is_hidden ? "primary" : "danger"}
                    onClick={() => handleToggleHidden(p.id, !p.is_hidden)}
                  >
                    {p.is_hidden ? "Mostra" : "Nascondi"}
                  </Button>
                  {rowError[p.id] && <p className="mt-1 text-xs text-red-700">{rowError[p.id]}</p>}
                </td>
              </tr>
            ))}
            {posts !== null && posts.length === 0 && (
              <tr>
                <td colSpan={5} className="px-4 py-6 text-center text-muted">
                  Nessun post trovato.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
