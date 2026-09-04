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
import type { SensitivityCategory } from "@/lib/content-media";
import type { Post, PostNote, PostTranslationSummary } from "@/lib/types";

const FORM_ID = "edit-post-form";

function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

/** Stato di pubblicazione nella toolbar, stesso stile di CategorySelect. La
 * sola transizione possibile da qui è bozza → pubblicato (non c'è un modo
 * per tornare a bozza una volta pubblicato): una volta pubblicato il select
 * mostra una singola opzione disattivata, non un vero multi-stato. */
function PostStatusSelect({ status, onPublish }: { status: Post["status"]; onPublish: () => void }) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-[11px] font-medium uppercase tracking-wide text-muted">Stato</span>
      <select
        value={status}
        disabled={status === "published"}
        onChange={(e) => {
          if (e.target.value === "published") onPublish();
        }}
        className="rounded-lg border border-border bg-background px-3 py-2 text-sm font-medium text-foreground transition hover:border-primary/50 focus:outline-none focus:ring-2 focus:ring-primary/40 disabled:cursor-not-allowed disabled:opacity-70"
      >
        <option value="draft">Bozza</option>
        <option value="published">{status === "published" ? "Pubblicato" : "Pubblica ora"}</option>
      </select>
    </label>
  );
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
  const [coverImageCategories, setCoverImageCategories] = useState<SensitivityCategory[]>([]);
  const [tags, setTags] = useState<string[]>([]);
  const [categoryId, setCategoryId] = useState<string | null>(null);
  const [notes, setNotes] = useState<PostNote[]>([]);
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
        setCoverImageCategories(p.cover_image_categories);
        setTags(p.manual_tags);
        setCategoryId(p.category?.id ?? null);
        setNotes(p.notes);
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
          cover_image_categories: coverImageCategories,
          tags,
          category_id: categoryId,
          notes,
        })
      );
      setPost(updated);
      setNotes(updated.notes);
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
    notes?: PostNote[];
  }) {
    const translated = await authFetch((token) =>
      api.posts.addTranslation(token, params.postId, { ...payload, notes: payload.notes ?? [] })
    );
    router.push(`/dashboard/blogs/${params.slug}/posts/${translated.id}`);
  }

  if (!post) return error ? <Alert kind="error">{error}</Alert> : <p className="text-sm text-muted">Caricamento…</p>;

  return (
    <div className="mx-auto max-w-2xl">
      <div className="mb-6 flex flex-wrap items-center justify-between gap-y-2">
        <Link href={`/dashboard/blogs/${params.slug}`} className="text-sm text-muted hover:text-foreground">
          ← Torna al blog
        </Link>
        <div className="flex flex-wrap items-center gap-4">
          {post.status === "published" && (
            <Link href={post.permalink} className="text-sm text-primary hover:underline">
              Vedi
            </Link>
          )}
          <Button type="submit" form={FORM_ID} variant="secondary" disabled={saving}>
            {saving ? "Salvataggio…" : "Salva"}
          </Button>
        </div>
      </div>

      <form id={FORM_ID} onSubmit={handleSave}>
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          required
          className="mb-8 w-full border-0 bg-transparent font-serif text-3xl font-semibold leading-tight text-foreground placeholder:text-muted/70 focus:outline-none sm:text-5xl"
        />

        <div className="mb-8">
          <TagInput value={tags} onChange={setTags} />
        </div>

        <div className="mb-8">
          <CoverImageUpload
            value={coverImageUrl}
            isSensitive={coverImageIsSensitive}
            categories={coverImageCategories}
            onChange={(url, sensitive, categories) => {
              setCoverImageUrl(url);
              setCoverImageIsSensitive(sensitive);
              setCoverImageCategories(categories);
            }}
            blogSlug={params.slug}
            authFetch={authFetch}
          />
        </div>

        <RichTextEditor
          value={content}
          onChange={setContent}
          blogSlug={params.slug}
          authFetch={authFetch}
          notes={notes}
          onNotesChange={setNotes}
          toolbarEnd={
            <>
              <CategorySelect blogSlug={params.slug} value={categoryId} onChange={setCategoryId} />
              <PostStatusSelect status={post.status} onPublish={handlePublish} />
            </>
          }
        />

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
        currentId={post.id}
        currentLocale={post.locale}
        blogSlug={params.slug}
        translations={translations}
        hrefFor={(id) => `/dashboard/blogs/${params.slug}/posts/${id}`}
        suggestedLocales={fallbackLanguages}
        authFetch={authFetch}
        onAddTranslation={handleAddTranslation}
      />
    </div>
  );
}
