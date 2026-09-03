"use client";

import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { RichTextEditor } from "@/components/editor/RichTextEditor";
import { TranslationsBar } from "@/components/editor/TranslationsBar";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { Page, PageTranslationSummary } from "@/lib/types";

function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

export default function AdminPagesPage() {
  const { accessToken } = useAuth();
  const [locale, setLocale] = useState("it");
  const [pages, setPages] = useState<Page[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);

  const load = useCallback(() => {
    api.pages
      .list(accessToken, locale)
      .then(setPages)
      .catch((err) => setError(errorMessage(err)));
  }, [accessToken, locale]);

  useEffect(load, [load]);

  return (
    <div>
      <div className="mb-6 flex items-center justify-between">
        <h1 className="font-serif text-2xl text-foreground">Pagine statiche</h1>
        <div className="flex items-center gap-3">
          <Label htmlFor="locale-filter">Lingua</Label>
          <Input
            id="locale-filter"
            className="w-16"
            maxLength={2}
            value={locale}
            onChange={(e) => setLocale(e.target.value.toLowerCase())}
          />
          <Button onClick={() => setShowCreate((s) => !s)}>{showCreate ? "Annulla" : "Nuova pagina"}</Button>
        </div>
      </div>

      {error && <Alert kind="error">{error}</Alert>}

      {showCreate && (
        <CreatePageForm
          defaultLocale={locale}
          onCreated={() => {
            setShowCreate(false);
            load();
          }}
        />
      )}

      <div className="space-y-3">
        {pages?.map((page) => (
          <Card key={page.id}>
            <div className="flex items-center justify-between">
              <div>
                <p className="font-serif text-lg text-foreground">{page.title}</p>
                <p className="text-sm text-muted">
                  /{page.slug} · {page.locale} · {page.is_published ? "pubblicata" : "bozza"}
                </p>
              </div>
              <Button variant="secondary" onClick={() => setEditingId(editingId === page.id ? null : page.id)}>
                {editingId === page.id ? "Chiudi" : "Modifica"}
              </Button>
            </div>
            {editingId === page.id && <EditPageForm page={page} onSaved={load} />}
          </Card>
        ))}
        {pages !== null && pages.length === 0 && (
          <p className="text-sm text-muted">Nessuna pagina per la lingua &quot;{locale}&quot;.</p>
        )}
      </div>
    </div>
  );
}

function CreatePageForm({ defaultLocale, onCreated }: { defaultLocale: string; onCreated: () => void }) {
  const { authFetch } = useAuth();
  const [slug, setSlug] = useState("");
  const [locale, setLocale] = useState(defaultLocale);
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [isPublished, setIsPublished] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    setError(null);
    try {
      await authFetch((token) =>
        api.pages.create(token, { slug, locale, title, content, is_published: isPublished })
      );
      onCreated();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  return (
    <Card className="mb-6">
      <CardTitle>Nuova pagina</CardTitle>
      <form onSubmit={handleSubmit}>
        <div className="grid gap-4 sm:grid-cols-2">
          <FieldGroup>
            <Label htmlFor="new-page-slug">Slug</Label>
            <Input id="new-page-slug" required value={slug} onChange={(e) => setSlug(e.target.value)} />
          </FieldGroup>
          <FieldGroup>
            <Label htmlFor="new-page-locale">Lingua</Label>
            <Input
              id="new-page-locale"
              required
              maxLength={2}
              value={locale}
              onChange={(e) => setLocale(e.target.value.toLowerCase())}
            />
          </FieldGroup>
        </div>
        <FieldGroup>
          <Label htmlFor="new-page-title">Titolo</Label>
          <Input id="new-page-title" required value={title} onChange={(e) => setTitle(e.target.value)} />
        </FieldGroup>
        <div className="mb-4">
          <RichTextEditor value={content} onChange={setContent} authFetch={authFetch} stickyToolbar={false} />
        </div>
        <FieldGroup>
          <label className="flex items-center gap-2 text-sm text-foreground">
            <input type="checkbox" checked={isPublished} onChange={(e) => setIsPublished(e.target.checked)} />
            Pubblica subito
          </label>
        </FieldGroup>
        {error && (
          <div className="mb-4">
            <Alert kind="error">{error}</Alert>
          </div>
        )}
        <Button type="submit">Crea</Button>
      </form>
    </Card>
  );
}

function EditPageForm({ page, onSaved }: { page: Page; onSaved: () => void }) {
  const { authFetch } = useAuth();
  const [title, setTitle] = useState(page.title);
  const [content, setContent] = useState(page.content);
  const [isPublished, setIsPublished] = useState(page.is_published);
  const [error, setError] = useState<string | null>(null);

  const [translations, setTranslations] = useState<PageTranslationSummary[]>([]);

  useEffect(() => {
    api.pages.translations(page.id).then(setTranslations).catch(() => undefined);
  }, [page.id]);

  async function handleSave(event: FormEvent) {
    event.preventDefault();
    setError(null);
    try {
      await authFetch((token) => api.pages.update(token, page.id, { title, content, is_published: isPublished }));
      onSaved();
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
    await authFetch((token) => api.pages.addTranslation(token, page.id, { ...payload, is_published: false }));
    onSaved();
  }

  return (
    <div className="mt-4 border-t border-border pt-4">
      <form onSubmit={handleSave}>
        <FieldGroup>
          <Label htmlFor={`edit-title-${page.id}`}>Titolo</Label>
          <Input id={`edit-title-${page.id}`} value={title} onChange={(e) => setTitle(e.target.value)} />
        </FieldGroup>
        <div className="mb-4">
          <RichTextEditor value={content} onChange={setContent} authFetch={authFetch} stickyToolbar={false} />
        </div>
        <FieldGroup>
          <label className="flex items-center gap-2 text-sm text-foreground">
            <input type="checkbox" checked={isPublished} onChange={(e) => setIsPublished(e.target.checked)} />
            Pubblicata
          </label>
        </FieldGroup>
        {error && (
          <div className="mb-4">
            <Alert kind="error">{error}</Alert>
          </div>
        )}
        <Button type="submit">Salva</Button>
      </form>

      <TranslationsBar
        currentId={page.id}
        currentLocale={page.locale}
        translations={translations}
        hrefFor={() => "/admin/pages"}
        suggestedLocales={[]}
        authFetch={authFetch}
        withNotes={false}
        onAddTranslation={handleAddTranslation}
      />
    </div>
  );
}
