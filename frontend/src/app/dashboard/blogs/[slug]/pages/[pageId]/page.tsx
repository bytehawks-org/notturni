"use client";

import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { RichTextEditor } from "@/components/editor/RichTextEditor";
import { TranslationsBar } from "@/components/editor/TranslationsBar";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { Page, PageTranslationSummary } from "@/lib/types";

const FORM_ID = "edit-blog-page-form";

function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

/** Editor di una pagina statica del blog — stessa interfaccia delle pagine
 * di piattaforma (frontend/src/app/admin/pages): niente tag/categorie,
 * editor senza il pulsante "Nota" (CLAUDE.md #1). */
export default function BlogPageEditorPage() {
  const params = useParams<{ slug: string; pageId: string }>();
  const router = useRouter();
  const { user, accessToken, authFetch } = useAuth();

  const [page, setPage] = useState<Page | null>(null);
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [isPublished, setIsPublished] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  const [translations, setTranslations] = useState<PageTranslationSummary[]>([]);
  const [fallbackLanguages, setFallbackLanguages] = useState<string[]>([]);

  // Stesse lingue di fallback del profilo usate dall'editor dei post (dashboard/profilo).
  useEffect(() => {
    if (!user) return;
    api.users
      .profile(user.username)
      .then((p) => setFallbackLanguages(p.fallback_languages))
      .catch(() => undefined);
  }, [user]);

  const load = useCallback(() => {
    api.blogs
      .getPageById(accessToken, params.slug, params.pageId)
      .then((found) => {
        setPage(found);
        setTitle(found.title);
        setContent(found.content);
        setIsPublished(found.is_published);
      })
      .catch((err) => setError(errorMessage(err)));
    api.blogs.pageTranslations(params.slug, params.pageId).then(setTranslations).catch(() => undefined);
  }, [params.slug, params.pageId, accessToken]);

  useEffect(load, [load]);

  async function handleSave(event: FormEvent) {
    event.preventDefault();
    setSaving(true);
    setError(null);
    try {
      const updated = await authFetch((token) =>
        api.blogs.updatePage(token, params.slug, params.pageId, { title, content, is_published: isPublished })
      );
      setPage(updated);
      setSaved(true);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSaving(false);
    }
  }

  async function handleDelete() {
    if (!window.confirm("Eliminare definitivamente questa pagina?")) return;
    try {
      await authFetch((token) => api.blogs.deletePage(token, params.slug, params.pageId));
      router.push(`/dashboard/blogs/${params.slug}`);
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
    const translated = await authFetch((token) =>
      api.blogs.addPageTranslation(token, params.slug, params.pageId, { ...payload, is_published: false })
    );
    router.push(`/dashboard/blogs/${params.slug}/pages/${translated.id}`);
  }

  if (!page) return error ? <Alert kind="error">{error}</Alert> : <p className="text-sm text-muted">Caricamento…</p>;

  return (
    <div className="mx-auto max-w-2xl">
      <div className="mb-10 flex items-center justify-between">
        <Link href={`/dashboard/blogs/${params.slug}`} className="text-sm text-muted hover:text-foreground">
          ← Torna al blog
        </Link>
        <div className="flex items-center gap-4">
          {page.is_published && page.permalink && (
            <Link href={page.permalink} className="text-sm text-primary hover:underline">
              Vedi
            </Link>
          )}
          <span className="text-sm text-muted">{page.is_published ? "Pubblicata" : "Bozza"}</span>
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
          className="mb-8 w-full border-0 bg-transparent font-serif text-5xl font-semibold leading-tight text-foreground placeholder:text-muted/70 focus:outline-none"
        />

        <RichTextEditor value={content} onChange={setContent} blogSlug={params.slug} authFetch={authFetch} />

        <div className="mt-6">
          <label className="flex items-center gap-2 text-sm text-foreground">
            <input type="checkbox" checked={isPublished} onChange={(e) => setIsPublished(e.target.checked)} />
            Pubblicata
          </label>
        </div>

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
        currentId={page.id}
        currentLocale={page.locale}
        blogSlug={params.slug}
        translations={translations}
        hrefFor={(id) => `/dashboard/blogs/${params.slug}/pages/${id}`}
        suggestedLocales={fallbackLanguages}
        authFetch={authFetch}
        withNotes={false}
        onAddTranslation={handleAddTranslation}
      />

      <div className="mt-10 border-t border-border pt-6">
        <button type="button" onClick={handleDelete} className="text-sm text-red-700 hover:underline">
          Elimina pagina
        </button>
      </div>
    </div>
  );
}
