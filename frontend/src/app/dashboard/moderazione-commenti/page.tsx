"use client";

import { useCallback, useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { type AdminComment, type CommentStatus } from "@/lib/types";

function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

export default function DashboardCommentModerationPage() {
  const { authFetch } = useAuth();
  const [status, setStatus] = useState<CommentStatus>("pending");
  const [comments, setComments] = useState<AdminComment[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [rowError, setRowError] = useState<Record<string, string>>({});

  const load = useCallback(() => {
    authFetch((token) => api.admin.listComments(token, { status }))
      .then(setComments)
      .catch((err) => setError(errorMessage(err)));
  }, [authFetch, status]);

  useEffect(load, [load]);

  async function handleModerate(commentId: string, action: "approve" | "reject") {
    setRowError((prev) => ({ ...prev, [commentId]: "" }));
    try {
      await authFetch((token) =>
        action === "approve" ? api.comments.approve(token, commentId) : api.comments.reject(token, commentId)
      );
      setComments((prev) => prev?.filter((c) => c.id !== commentId) ?? null);
    } catch (err) {
      setRowError((prev) => ({ ...prev, [commentId]: errorMessage(err) }));
    }
  }

  return (
    <div>
      <div className="mb-2 flex items-center justify-between">
        <h1 className="font-serif text-2xl text-foreground">Moderazione commenti</h1>
        <select
          value={status}
          onChange={(e) => setStatus(e.target.value as CommentStatus)}
          aria-label="Filtra per stato"
          className="max-w-xs rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-primary/40"
        >
          <option value="pending">In attesa</option>
          <option value="approved">Approvati</option>
          <option value="rejected">Rifiutati</option>
        </select>
      </div>
      <p className="mb-6 text-sm text-muted">
        Commenti di tutti i blog della piattaforma, indipendentemente da chi ne è proprietario o
        mediatore.
      </p>
      {error && <Alert kind="error">{error}</Alert>}

      <div className="overflow-x-auto rounded-lg border border-border">
        <table className="w-full text-left text-sm">
          <thead className="border-b border-border text-muted">
            <tr>
              <th className="px-4 py-3">Blog</th>
              <th className="px-4 py-3">Post</th>
              <th className="px-4 py-3">Autore</th>
              <th className="px-4 py-3">Contenuto</th>
              {status === "pending" && <th className="px-4 py-3">Moderazione</th>}
            </tr>
          </thead>
          <tbody>
            {comments?.map((c) => (
              <tr key={c.id} className="border-b border-border align-top last:border-0">
                <td className="px-4 py-3 whitespace-nowrap text-muted">{c.blog_slug}</td>
                <td className="px-4 py-3 text-muted">{c.post_title}</td>
                <td className="px-4 py-3 text-muted">{c.author_display_name}</td>
                <td className="px-4 py-3 text-foreground">{c.content}</td>
                {status === "pending" && (
                  <td className="px-4 py-3">
                    <div className="flex gap-2">
                      <Button onClick={() => handleModerate(c.id, "approve")}>Approva</Button>
                      <Button variant="danger" onClick={() => handleModerate(c.id, "reject")}>
                        Rifiuta
                      </Button>
                    </div>
                    {rowError[c.id] && <p className="mt-1 text-xs text-red-700">{rowError[c.id]}</p>}
                  </td>
                )}
              </tr>
            ))}
            {comments !== null && comments.length === 0 && (
              <tr>
                <td colSpan={status === "pending" ? 5 : 4} className="px-4 py-6 text-center text-muted">
                  Nessun commento in questo stato.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
