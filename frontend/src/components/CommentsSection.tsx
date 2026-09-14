"use client";

import { useLocale, useTranslations } from "next-intl";
import Link from "next/link";
import Script from "next/script";
import { useEffect, useMemo, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { FieldGroup, Input, Label, TextArea } from "@/components/ui/Field";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import type { Comment, CommentsMode } from "@/lib/types";

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
  const t = useTranslations("CommentsSection");
  const tc = useTranslations("Common");
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
      setError(err instanceof ApiClientError ? err.message : tc("unexpectedError"));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-3">
      {needsAnonymousFields && (
        <div className="flex flex-wrap gap-3">
          <div className="flex-1">
            <Label htmlFor={`author-${parentId ?? "top"}`}>{t("name")}</Label>
            <Input
              id={`author-${parentId ?? "top"}`}
              required
              value={authorDisplayName}
              onChange={(e) => setAuthorDisplayName(e.target.value)}
            />
          </div>
          <div className="flex-1">
            <Label htmlFor={`email-${parentId ?? "top"}`}>{t("emailNotPublished")}</Label>
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
      <FieldGroup className="mb-0">
        <TextArea
          required
          placeholder={parentId ? t("replyPlaceholder") : t("placeholder")}
          value={content}
          onChange={(e) => setContent(e.target.value)}
        />
      </FieldGroup>
      {needsAnonymousFields && turnstileSiteKey && <div className="cf-turnstile" data-sitekey={turnstileSiteKey} />}
      {error && <Alert kind="error">{error}</Alert>}
      <div className="flex gap-2">
        <Button type="submit" size="sm" disabled={submitting}>
          {submitting ? t("sending") : t("send")}
        </Button>
        {onCancel && (
          <Button type="button" variant="secondary" size="sm" onClick={onCancel}>
            {tc("cancel")}
          </Button>
        )}
      </div>
    </form>
  );
}

function CommentBody({ comment }: { comment: Comment }) {
  const locale = useLocale();
  return (
    <>
      <div className="flex items-center gap-2 text-[13px]">
        <span className="grid h-6 w-6 place-items-center rounded-full bg-primary/15 font-serif text-xs text-primary">
          {comment.author_display_name[0]?.toUpperCase()}
        </span>
        <span className="font-medium text-foreground">{comment.author_display_name}</span>
        <span className="text-muted">· {formatDate(comment.created_at, locale)}</span>
      </div>
      <p className="mt-2 whitespace-pre-wrap text-[15px] leading-relaxed text-foreground/90">{comment.content}</p>
    </>
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
  const t = useTranslations("CommentsSection");
  const tc = useTranslations("Common");
  const [replying, setReplying] = useState(false);

  return (
    <div className="border-b border-border py-5 last:border-0">
      <CommentBody comment={comment} />
      {mode !== "closed" && (
        <button type="button" onClick={() => setReplying((v) => !v)} className="mt-2 text-[13px] text-primary hover:underline">
          {replying ? tc("cancel") : t("reply")}
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
        <div className="mt-4 ml-4 space-y-4 border-l border-border pl-4">
          {replies.map((reply) => (
            <div key={reply.id}>
              <CommentBody comment={reply} />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/** Commenti del post (mockup 1a): titolo con conteggio, riga di policy,
 * form/inviti ad accedere, elenco con risposte a un livello. */
export function CommentsSection({ postId, mode }: { postId: string; mode: CommentsMode }) {
  const { user } = useAuth();
  const t = useTranslations("CommentsSection");
  const tc = useTranslations("Common");
  const [comments, setComments] = useState<Comment[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [turnstileSiteKey, setTurnstileSiteKey] = useState<string | null>(null);

  useEffect(() => {
    api.comments
      .listApproved(postId)
      .then(setComments)
      .catch((err) => setError(err instanceof ApiClientError ? err.message : tc("unexpectedError")));
  }, [postId, tc]);

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

  const policy = mode === "closed" ? t("policyClosed") : mode === "members" ? t("policyMembers") : t("policyEveryone");

  return (
    <section className="mt-16 border-t border-border pt-8">
      <div className="mb-5 flex flex-wrap items-baseline justify-between gap-2">
        <h2 className="font-serif text-2xl text-foreground">
          {t("title")}
          {comments !== null && <span className="text-muted"> · {comments.length}</span>}
        </h2>
        <span className="text-[13px] text-muted">{policy}</span>
      </div>
      {mode === "everyone" && (
        <Script src="https://challenges.cloudflare.com/turnstile/v0/api.js" strategy="lazyOnload" async defer />
      )}
      {error && <Alert kind="error">{error}</Alert>}

      {mode === "members" && !user && (
        <p className="rounded-xl border border-border bg-surface px-4 py-3 text-sm text-muted">
          <Link href="/login" className="font-medium text-primary no-underline hover:underline">
            {t("signIn")}
          </Link>{" "}
          {t("toComment")}
        </p>
      )}

      {mode !== "closed" && (mode === "everyone" || user) && (
        <div className="mb-8">
          <CommentForm postId={postId} parentId={null} mode={mode} turnstileSiteKey={turnstileSiteKey} onPosted={handlePosted} />
        </div>
      )}

      {comments !== null && topLevel.length === 0 && <p className="text-sm text-muted">{t("none")}</p>}

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
