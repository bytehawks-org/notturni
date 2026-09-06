"use client";

import Link from "next/link";
import Script from "next/script";
import { useEffect, useMemo, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { FieldGroup, Input, Label, TextArea } from "@/components/ui/Field";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { Comment, CommentsMode } from "@/lib/types";

function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString("it-IT", { day: "numeric", month: "long", year: "numeric" });
}

interface CommentFormProps {
  postId: string;
  parentId: string | null;
  mode: CommentsMode;
  turnstileSiteKey: string | null;
  onPosted: (comment: Comment) => void;
  onCancel?: () => void;
}

function CommentForm({ postId, parentId, mode, turnstileSiteKey, onPosted, onCancel }: CommentFormProps) {
  const { user, accessToken } = useAuth();
  const [content, setContent] = useState("");
  const [authorDisplayName, setAuthorDisplayName] = useState("");
  const [authorEmail, setAuthorEmail] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const needsAnonymousFields = !user && mode === "everyone";

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      const captchaToken = needsAnonymousFields
        ? (new FormData(event.currentTarget).get("cf-turnstile-response") as string | null)
        : undefined;
      const comment = await api.comments.create(user ? accessToken : null, postId, {
        content,
        parent_id: parentId ?? undefined,
        author_display_name: needsAnonymousFields ? authorDisplayName : undefined,
        author_email: needsAnonymousFields ? authorEmail : undefined,
        captcha_token: captchaToken ?? undefined,
      });
      setContent("");
      setAuthorDisplayName("");
      setAuthorEmail("");
      onPosted(comment);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-3">
      {needsAnonymousFields && (
        <div className="flex flex-wrap gap-3">
          <div className="flex-1">
            <Label htmlFor={`author-${parentId ?? "top"}`}>Nome</Label>
            <Input
              id={`author-${parentId ?? "top"}`}
              required
              value={authorDisplayName}
              onChange={(e) => setAuthorDisplayName(e.target.value)}
            />
          </div>
          <div className="flex-1">
            <Label htmlFor={`email-${parentId ?? "top"}`}>Email (non pubblicata)</Label>
            <Input
              id={`email-${parentId ?? "top"}`}
              type="email"
              required
              value={authorEmail}
              onChange={(e) => setAuthorEmail(e.target.value)}
            />
          </div>
        </div>
      )}
      <FieldGroup>
        <TextArea
          required
          placeholder={parentId ? "Scrivi una risposta…" : "Scrivi un commento…"}
          value={content}
          onChange={(e) => setContent(e.target.value)}
        />
      </FieldGroup>
      {needsAnonymousFields && turnstileSiteKey && (
        <div className="cf-turnstile" data-sitekey={turnstileSiteKey} />
      )}
      {error && <Alert kind="error">{error}</Alert>}
      <div className="flex gap-2">
        <Button type="submit" disabled={submitting}>
          {submitting ? "Invio…" : "Invia"}
        </Button>
        {onCancel && (
          <Button type="button" variant="secondary" onClick={onCancel}>
            Annulla
          </Button>
        )}
      </div>
    </form>
  );
}

function CommentItem({
  comment,
  replies,
  postId,
  mode,
  turnstileSiteKey,
  onPosted,
}: {
  comment: Comment;
  replies: Comment[];
  postId: string;
  mode: CommentsMode;
  turnstileSiteKey: string | null;
  onPosted: (comment: Comment) => void;
}) {
  const [replying, setReplying] = useState(false);

  return (
    <div className="border-b border-border/60 py-5 last:border-0">
      <p className="text-sm font-medium text-foreground">{comment.author_display_name}</p>
      <p className="text-xs text-muted">{formatDate(comment.created_at)}</p>
      <p className="mt-2 whitespace-pre-wrap text-sm leading-relaxed text-foreground/90">{comment.content}</p>
      {mode !== "closed" && (
        <button
          type="button"
          onClick={() => setReplying((v) => !v)}
          className="mt-2 text-xs text-primary hover:underline"
        >
          {replying ? "Annulla" : "Rispondi"}
        </button>
      )}
      {replying && (
        <div className="mt-3">
          <CommentForm
            postId={postId}
            parentId={comment.id}
            mode={mode}
            turnstileSiteKey={turnstileSiteKey}
            onPosted={(c) => {
              onPosted(c);
              setReplying(false);
            }}
            onCancel={() => setReplying(false)}
          />
        </div>
      )}
      {replies.length > 0 && (
        <div className="mt-4 ml-6 space-y-4 border-l border-border/60 pl-4">
          {replies.map((reply) => (
            <div key={reply.id}>
              <p className="text-sm font-medium text-foreground">{reply.author_display_name}</p>
              <p className="text-xs text-muted">{formatDate(reply.created_at)}</p>
              <p className="mt-2 whitespace-pre-wrap text-sm leading-relaxed text-foreground/90">
                {reply.content}
              </p>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export function CommentsSection({ postId, mode }: { postId: string; mode: CommentsMode }) {
  const { user } = useAuth();
  const [comments, setComments] = useState<Comment[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [turnstileSiteKey, setTurnstileSiteKey] = useState<string | null>(null);

  useEffect(() => {
    api.comments
      .listApproved(postId)
      .then(setComments)
      .catch((err) => setError(errorMessage(err)));
  }, [postId]);

  useEffect(() => {
    if (mode !== "everyone") return;
    api.config
      .get()
      .then((config) => setTurnstileSiteKey(config.turnstile_site_key))
      .catch(() => undefined);
  }, [mode]);

  const { topLevel, repliesByParent } = useMemo(() => {
    const top: Comment[] = [];
    const byParent = new Map<string, Comment[]>();
    for (const c of comments ?? []) {
      if (c.parent_id === null) {
        top.push(c);
      } else {
        const list = byParent.get(c.parent_id) ?? [];
        list.push(c);
        byParent.set(c.parent_id, list);
      }
    }
    return { topLevel: top, repliesByParent: byParent };
  }, [comments]);

  function handlePosted(comment: Comment) {
    // I commenti anonimi restano in moderazione (pending): non compaiono
    // subito nella lista, che mostra solo gli approvati.
    if (comment.status === "approved") {
      setComments((prev) => [...(prev ?? []), comment]);
    }
  }

  return (
    <section className="mt-16 border-t border-border pt-10">
      <h2 className="mb-6 font-serif text-2xl text-foreground">Commenti</h2>
      {mode === "everyone" && (
        <Script src="https://challenges.cloudflare.com/turnstile/v0/api.js" strategy="lazyOnload" async defer />
      )}
      {error && <Alert kind="error">{error}</Alert>}

      {mode === "closed" && <p className="text-sm text-muted">I commenti sono chiusi per questo post.</p>}

      {mode === "members" && !user && (
        <p className="text-sm text-muted">
          <Link href="/login" className="text-primary hover:underline">
            Accedi
          </Link>{" "}
          per lasciare un commento.
        </p>
      )}

      {mode !== "closed" && (mode === "everyone" || user) && (
        <div className="mb-8">
          <CommentForm postId={postId} parentId={null} mode={mode} turnstileSiteKey={turnstileSiteKey} onPosted={handlePosted} />
        </div>
      )}

      {comments !== null && topLevel.length === 0 && (
        <p className="text-sm text-muted">Nessun commento, per ora.</p>
      )}

      <div>
        {topLevel.map((comment) => (
          <CommentItem
            key={comment.id}
            comment={comment}
            replies={repliesByParent.get(comment.id) ?? []}
            postId={postId}
            mode={mode}
            turnstileSiteKey={turnstileSiteKey}
            onPosted={handlePosted}
          />
        ))}
      </div>
    </section>
  );
}
