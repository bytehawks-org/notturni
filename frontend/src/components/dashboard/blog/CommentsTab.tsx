"use client";

import { useCallback, useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { type BlogComment } from "@/lib/types";

import { errorMessage } from "./shared";

export function CommentsTab({ blogSlug, canModerate }: { blogSlug: string; canModerate: boolean }) {
  const { accessToken, authFetch } = useAuth();
  const [pending, setPending] = useState<BlogComment[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!canModerate || !accessToken) {
      setPending([]);
      return;
    }
    try {
      // Un'unica richiesta aggregata per tutto il blog, non una per post.
      setPending(await authFetch((token) => api.comments.listForBlog(token, blogSlug)));
    } catch (err) {
      setError(errorMessage(err));
    }
  }, [blogSlug, canModerate, accessToken, authFetch]);

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- fetch dati all'apertura della tab
    load();
  }, [load]);

  async function handleModerate(commentId: string, action: "approve" | "reject") {
    try {
      await authFetch((token) =>
        action === "approve" ? api.comments.approve(token, commentId) : api.comments.reject(token, commentId)
      );
      setPending((prev) => prev?.filter((c) => c.id !== commentId) ?? null);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  if (!canModerate) return <p className="text-sm text-muted">Solo il proprietario può moderare i commenti.</p>;

  return (
    <div>
      {error && <Alert kind="error">{error}</Alert>}
      {pending !== null && pending.length === 0 && (
        <p className="text-sm text-muted">Nessun commento in attesa di moderazione.</p>
      )}
      <div className="space-y-3">
        {pending?.map((comment) => (
          <Card key={comment.id}>
            <p className="text-xs text-muted">
              su <span className="text-foreground">{comment.post_title}</span> — da{" "}
              {comment.author_display_name}
            </p>
            <p className="my-2 text-sm text-foreground">{comment.content}</p>
            <div className="flex gap-2">
              <Button onClick={() => handleModerate(comment.id, "approve")}>Approva</Button>
              <Button variant="danger" onClick={() => handleModerate(comment.id, "reject")}>
                Rifiuta
              </Button>
            </div>
          </Card>
        ))}
      </div>
    </div>
  );
}
