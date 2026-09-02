"use client";

import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { CategorySelect } from "@/components/editor/CategorySelect";
import { CoverImageUpload } from "@/components/editor/CoverImageUpload";
import { RichTextEditor } from "@/components/editor/RichTextEditor";
import { TagInput } from "@/components/editor/TagInput";
import { TranslationsBar } from "@/components/editor/TranslationsBar";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { Post, PostTranslationSummary } from "@/lib/types";

const FORM_ID = "edit-post-form";

function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

export default function PostEditorPage() {
  const params = useParams<{ slug: string; postId: string }>();
  const router = useRouter();
  const { user, accessToken, authFetch } = useAuth();

  const [post, setPost] = useState<Post | null>(null);
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [coverImageUrl, setCoverImageUrl] = useState<string | null>(null);
  const [coverImageIsSensitive, setCoverImageIsSensitive] = useState(false);
  const [tags, setTags] = useState<string[]>([]);
  const [categoryId, setCategoryId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  const [translations, setTranslations] = useState<PostTranslationSummary[]>([]);
  const [fallbackLanguages, setFallbackLanguages] = useState<string[]>([]);

  const load = useCallback(() => {
    api.posts
      .get(accessToken, params.postId)
      .then((p) => {
        setPost(p);
        setTitle(p.title);
        setContent(p.content);
        setCoverImageUrl(p.cover_image_url);
        setCoverImageIsSensitive(p.cover_image_is_sensitive);
        setTags(p.manual_tags);
        setCategoryId(p.category?.id ?? null);
      })
      .catch((err) => setError(errorMessage(err)));
    api.posts.translations(params.postId).then(setTranslations).catch(() => undefined);
  }, [params.postId, accessToken]);

  // Le lingue di fallback del profilo (vedi dashboard/profile) popolano il
  // selettore lingua in "Aggiungi traduzione", invece di dover scrivere la sigla.
  useEffect(() => {
    if (!user) return;
    api.users
      .profile(user.username)
      .then((p) => setFallbackLanguages(p.fallback_languages))
      .catch(() => undefined);
  }, [user]);

  useEffect(load, [load]);

  async function handleSave(event: FormEvent) {
    event.preventDefault();
    setSaving(true);
    setError(null);
    try {
      const updated = await authFetch((token) =>
        api.posts.update(token, params.postId, {
          title,
          content,
          cover_image_url: coverImageUrl ?? "",
          cover_image_is_sensitive: coverImageIsSensitive,
          tags,
          category_id: categoryId,
        })
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

  async function handleAddTranslation(payload: {
    slug: string;
    locale: string;
    title: string;
    content: string;
  }) {
    const translated = await authFetch((token) => api.posts.addTranslation(token, params.postId, payload));
    router.push(`/dashboard/blogs/${params.slug}/posts/${translated.id}`);
  }

  if (!post) return error ? <Alert kind="error">{error}</Alert> : <p className="text-sm text-muted">Caricamento…</p>;

  return (
    <div className="mx-auto max-w-2xl">
      <div className="mb-10 flex items-center justify-between">
        <Link href={`/dashboard/blogs/${params.slug}`} className="text-sm text-muted hover:text-foreground">
          ← Torna al blog
        </Link>
        <div className="flex items-center gap-4">
          {post.status === "published" && (
            <Link href={post.permalink} className="text-sm text-primary hover:underline">
              Vedi
            </Link>
          )}
          <span className="text-sm text-muted">{post.status === "published" ? "Pubblicato" : "Bozza"}</span>
          <Button type="submit" form={FORM_ID} variant="secondary" disabled={saving}>
            {saving ? "Salvataggio…" : "Salva"}
          </Button>
          {post.status === "draft" && (
            <Button type="button" onClick={handlePublish}>
              Pubblica
            </Button>
          )}
        </div>
      </div>

      <form id={FORM_ID} onSubmit={handleSave}>
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          required
          className="mb-8 w-full border-0 bg-transparent font-serif text-5xl font-semibold leading-tight text-foreground placeholder:text-muted/70 focus:outline-none"
        />

        <div className="mb-4">
          <CategorySelect blogSlug={params.slug} value={categoryId} onChange={setCategoryId} />
        </div>

        <div className="mb-8">
          <TagInput value={tags} onChange={setTags} />
        </div>

        <div className="mb-8">
          <CoverImageUpload
            value={coverImageUrl}
            isSensitive={coverImageIsSensitive}
            onChange={(url, sensitive) => {
              setCoverImageUrl(url);
              setCoverImageIsSensitive(sensitive);
            }}
            blogSlug={params.slug}
            authFetch={authFetch}
          />
        </div>

        <RichTextEditor value={content} onChange={setContent} blogSlug={params.slug} authFetch={authFetch} />

        {error && (
          <div className="mt-6">
            <Alert kind="error">{error}</Alert>
          </div>
        )}
        {saved && (
          <div className="mt-6">
            <Alert kind="success">Salvato.</Alert>
          </div>
        )}
      </form>

      <TranslationsBar
        currentPostId={post.id}
        currentLocale={post.locale}
        blogSlug={params.slug}
        translations={translations}
        suggestedLocales={fallbackLanguages}
        authFetch={authFetch}
        onAddTranslation={handleAddTranslation}
      />
    </div>
  );
}
