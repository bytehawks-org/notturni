"use client";

import { useCallback, useEffect, useState } from "react";

import { SearchInput } from "@/components/SearchInput";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { BLOG_VISIBILITY_LABELS, type AdminBlog } from "@/lib/types";

function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

export default function AdminBlogsPage() {
  const { authFetch } = useAuth();
  const [q, setQ] = useState("");
  const [blogs, setBlogs] = useState<AdminBlog[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [rowError, setRowError] = useState<Record<string, string>>({});

  const load = useCallback(() => {
    authFetch((token) => api.admin.listBlogs(token, q))
      .then(setBlogs)
      .catch((err) => setError(errorMessage(err)));
  }, [authFetch, q]);

  useEffect(load, [load]);

  async function handleToggleSuspended(blogId: string, is_suspended: boolean) {
    setRowError((prev) => ({ ...prev, [blogId]: "" }));
    try {
      const updated = await authFetch((token) => api.admin.updateBlog(token, blogId, { is_suspended }));
      setBlogs((prev) => prev?.map((b) => (b.id === blogId ? updated : b)) ?? null);
    } catch (err) {
      setRowError((prev) => ({ ...prev, [blogId]: errorMessage(err) }));
    }
  }

  return (
    <div>
      <div className="mb-6 flex items-center justify-between">
        <h1 className="font-serif text-2xl text-foreground">Blog</h1>
        <SearchInput value={q} onChange={setQ} placeholder="Cerca per slug, titolo o proprietario…" />
      </div>
      {error && <Alert kind="error">{error}</Alert>}

      <div className="overflow-x-auto rounded-lg border border-border">
        <table className="w-full text-left text-sm">
          <thead className="border-b border-border text-muted">
            <tr>
              <th className="px-4 py-3">Slug</th>
              <th className="px-4 py-3">Titolo</th>
              <th className="px-4 py-3">Proprietario</th>
              <th className="px-4 py-3">Visibilità</th>
              <th className="px-4 py-3">Stato</th>
            </tr>
          </thead>
          <tbody>
            {blogs?.map((b) => (
              <tr key={b.id} className="border-b border-border last:border-0">
                <td className="px-4 py-3 text-foreground">{b.slug}</td>
                <td className="px-4 py-3 text-foreground">{b.title}</td>
                <td className="px-4 py-3 text-muted">{b.owner_username}</td>
                <td className="px-4 py-3 text-muted">{BLOG_VISIBILITY_LABELS[b.visibility]}</td>
                <td className="px-4 py-3">
                  <Button
                    variant={b.is_suspended ? "primary" : "danger"}
                    onClick={() => handleToggleSuspended(b.id, !b.is_suspended)}
                  >
                    {b.is_suspended ? "Riattiva" : "Sospendi"}
                  </Button>
                  {rowError[b.id] && <p className="mt-1 text-xs text-red-700">{rowError[b.id]}</p>}
                </td>
              </tr>
            ))}
            {blogs !== null && blogs.length === 0 && (
              <tr>
                <td colSpan={5} className="px-4 py-6 text-center text-muted">
                  Nessun blog trovato.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
