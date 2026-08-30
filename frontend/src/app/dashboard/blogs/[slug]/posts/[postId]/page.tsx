"use client";

import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { CoverImageUpload } from "@/components/editor/CoverImageUpload";
import { RichTextEditor } from "@/components/editor/RichTextEditor";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { Post, PostTranslationSummary } from "@/lib/types";

function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

export default function PostEditorPage() {
  const params = useParams<{ slug: string; postId: string }>();
  const router = useRouter();
  const { accessToken, authFetch } = useAuth();

  const [post, setPost] = useState<Post | null>(null);
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [coverImageUrl, setCoverImageUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  const [translations, setTranslations] = useState<PostTranslationSummary[]>([]);
  const [showTranslate, setShowTranslate] = useState(false);
  const [trLocale, setTrLocale] = useState("");
  const [trSlug, setTrSlug] = useState("");
  const [trTitle, setTrTitle] = useState("");
  const [trContent, setTrContent] = useState("");
  const [trError, setTrError] = useState<string | null>(null);

  const load = useCallback(() => {
    api.posts
      .get(accessToken, params.postId)
      .then((p) => {
        setPost(p);
        setTitle(p.title);
        setContent(p.content);
        setCoverImageUrl(p.cover_image_url);
      })
      .catch((err) => setError(errorMessage(err)));
    api.posts.translations(params.postId).then(setTranslations).catch(() => undefined);
  }, [params.postId, accessToken]);

  useEffect(load, [load]);

  async function handleSave(event: FormEvent) {
    event.preventDefault();
    setSaving(true);
    setError(null);
    try {
      const updated = await authFetch((token) =>
        api.posts.update(token, params.postId, { title, content, cover_image_url: coverImageUrl ?? "" })
      );
      setPost(updated);
      setSaved(true);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSaving(false);
    }
  }

  async function handlePublish() {
    try {
      const updated = await authFetch((token) => api.posts.publish(token, params.postId));
      setPost(updated);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  async function handleAddTranslation(event: FormEvent) {
    event.preventDefault();
    setTrError(null);
    try {
      const translated = await authFetch((token) =>
        api.posts.addTranslation(token, params.postId, {
          slug: trSlug,
          locale: trLocale,
          title: trTitle,
          content: trContent,
        })
      );
      setShowTranslate(false);
      setTrLocale("");
      setTrSlug("");
      setTrTitle("");
      setTrContent("");
      router.push(`/dashboard/blogs/${params.slug}/posts/${translated.id}`);
    } catch (err) {
      setTrError(errorMessage(err));
    }
  }

  if (!post) return error ? <Alert kind="error">{error}</Alert> : <p className="text-sm text-muted">Caricamento…</p>;

  return (
    <div className="mx-auto max-w-2xl">
      <Link href={`/dashboard/blogs/${params.slug}`} className="text-sm text-muted hover:text-foreground">
        ← Torna al blog
      </Link>

      <Card className="mt-4">
        <div className="mb-4 flex items-center justify-between">
          <CardTitle>Modifica post ({post.locale})</CardTitle>
          <span className="text-sm text-muted">{post.status === "published" ? "Pubblicato" : "Bozza"}</span>
        </div>
        <form onSubmit={handleSave}>
          <input
            id="edit-title"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            required
            className="mb-4 w-full border-0 bg-transparent font-serif text-3xl font-semibold text-foreground placeholder:text-muted focus:outline-none"
          />

          <div className="mb-4">
            <CoverImageUpload
              value={coverImageUrl}
              onChange={setCoverImageUrl}
              blogSlug={params.slug}
              authFetch={authFetch}
            />
          </div>

          <div className="mb-4">
            <RichTextEditor
              value={content}
              onChange={setContent}
              blogSlug={params.slug}
              authFetch={authFetch}
            />
          </div>

          {error && (
            <div className="mb-4">
              <Alert kind="error">{error}</Alert>
            </div>
          )}
          {saved && (
            <div className="mb-4">
              <Alert kind="success">Salvato.</Alert>
            </div>
          )}
          <div className="flex gap-2">
            <Button type="submit" disabled={saving}>
              {saving ? "Salvataggio…" : "Salva"}
            </Button>
            {post.status === "draft" && (
              <Button type="button" variant="secondary" onClick={handlePublish}>
                Pubblica
              </Button>
            )}
          </div>
        </form>
      </Card>

      <Card className="mt-6">
        <CardTitle>Traduzioni</CardTitle>
        <ul className="mb-4 space-y-1">
          {translations.map((t) => (
            <li key={t.id} className="text-sm">
              {t.id === post.id ? (
                <span className="text-foreground">{t.locale} (questa) — {t.status}</span>
              ) : (
                <Link
                  href={`/dashboard/blogs/${params.slug}/posts/${t.id}`}
                  className="text-primary underline underline-offset-4"
                >
                  {t.locale} — {t.status}
                </Link>
              )}
            </li>
          ))}
        </ul>
        {!showTranslate ? (
          <Button variant="secondary" onClick={() => setShowTranslate(true)}>
            Aggiungi traduzione
          </Button>
        ) : (
          <form onSubmit={handleAddTranslation}>
            <FieldGroup>
              <Label htmlFor="tr-locale">Lingua (es. en, de)</Label>
              <Input
                id="tr-locale"
                required
                maxLength={2}
                minLength={2}
                value={trLocale}
                onChange={(e) => setTrLocale(e.target.value.toLowerCase())}
              />
            </FieldGroup>
            <FieldGroup>
              <Label htmlFor="tr-slug">Slug</Label>
              <Input id="tr-slug" required value={trSlug} onChange={(e) => setTrSlug(e.target.value)} />
            </FieldGroup>
            <FieldGroup>
              <Label htmlFor="tr-title">Titolo</Label>
              <Input id="tr-title" required value={trTitle} onChange={(e) => setTrTitle(e.target.value)} />
            </FieldGroup>
            <FieldGroup>
              <Label htmlFor="tr-content">Contenuto</Label>
              <RichTextEditor
                value={trContent}
                onChange={setTrContent}
                blogSlug={params.slug}
                authFetch={authFetch}
              />
            </FieldGroup>
            {trError && (
              <div className="mb-4">
                <Alert kind="error">{trError}</Alert>
              </div>
            )}
            <div className="flex gap-2">
              <Button type="submit">Crea traduzione</Button>
              <Button type="button" variant="secondary" onClick={() => setShowTranslate(false)}>
                Annulla
              </Button>
            </div>
          </form>
        )}
      </Card>
    </div>
  );
}
