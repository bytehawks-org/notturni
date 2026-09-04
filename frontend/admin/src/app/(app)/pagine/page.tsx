"use client";

import { useCallback, useEffect, useState, type FormEvent } from "react";

import { SearchInput } from "@/components/SearchInput";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { FieldGroup, Input, Label, TextArea } from "@/components/ui/Field";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { Page, PageTranslationSummary } from "@/lib/types";

function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

export default function AdminPagesPage() {
  const { accessToken } = useAuth();
  const [locale, setLocale] = useState("it");
  const [q, setQ] = useState("");
  const [pages, setPages] = useState<Page[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);

  const load = useCallback(() => {
    api.pages
      .list(accessToken, locale, q)
      .then(setPages)
      .catch((err) => setError(errorMessage(err)));
  }, [accessToken, locale, q]);

  useEffect(load, [load]);

  return (
    <div>
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <h1 className="font-serif text-2xl text-foreground">Pagine statiche</h1>
        <div className="flex items-center gap-3">
          <SearchInput value={q} onChange={setQ} placeholder="Cerca per titolo o slug…" />
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
          <div key={page.id} className="rounded-lg border border-border bg-background p-6">
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
          </div>
        ))}
        {pages !== null && pages.length === 0 && (
          <p className="text-sm text-muted">Nessuna pagina trovata per la lingua &quot;{locale}&quot;.</p>
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
    <div className="mb-6 rounded-lg border border-border bg-background p-6">
      <h2 className="mb-4 font-serif text-xl text-foreground">Nuova pagina</h2>
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
        <FieldGroup>
          <Label htmlFor="new-page-content">Contenuto (Markdown)</Label>
          <TextArea
            id="new-page-content"
            required
            rows={12}
            value={content}
            onChange={(e) => setContent(e.target.value)}
          />
        </FieldGroup>
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
    </div>
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

  return (
    <div className="mt-4 border-t border-border pt-4">
      <form onSubmit={handleSave}>
        <FieldGroup>
          <Label htmlFor={`edit-title-${page.id}`}>Titolo</Label>
          <Input id={`edit-title-${page.id}`} value={title} onChange={(e) => setTitle(e.target.value)} />
        </FieldGroup>
        <FieldGroup>
          <Label htmlFor={`edit-content-${page.id}`}>Contenuto (Markdown)</Label>
          <TextArea
            id={`edit-content-${page.id}`}
            rows={12}
            value={content}
            onChange={(e) => setContent(e.target.value)}
          />
        </FieldGroup>
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

      <AddTranslationForm page={page} translations={translations} onAdded={onSaved} />
    </div>
  );
}

function AddTranslationForm({
  page,
  translations,
  onAdded,
}: {
  page: Page;
  translations: PageTranslationSummary[];
  onAdded: () => void;
}) {
  const { authFetch } = useAuth();
  const [adding, setAdding] = useState(false);
  const [slug, setSlug] = useState("");
  const [locale, setLocale] = useState("");
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [error, setError] = useState<string | null>(null);

  const otherTranslations = translations.filter((t) => t.id !== page.id);

  async function handleAdd(event: FormEvent) {
    event.preventDefault();
    setError(null);
    try {
      await authFetch((token) => api.pages.addTranslation(token, page.id, { slug, locale, title, content, is_published: false }));
      setAdding(false);
      onAdded();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  return (
    <div className="mt-6 border-t border-border/60 pt-4">
      <div className="flex flex-wrap items-center gap-x-1.5 gap-y-2 text-sm">
        <span className="text-muted">Anche in</span>
        <span className="text-foreground underline underline-offset-4">{page.locale}</span>
        {otherTranslations.map((t) => (
          <span key={t.id} className="text-foreground/70">
            · {t.locale} ✓
          </span>
        ))}
        {!adding && (
          <button type="button" onClick={() => setAdding(true)} className="ml-1 text-primary hover:underline">
            + Aggiungi lingua
          </button>
        )}
      </div>

      {adding && (
        <form onSubmit={handleAdd} className="mt-4">
          <div className="mb-4 grid gap-4 sm:grid-cols-2">
            <FieldGroup>
              <Label htmlFor={`tr-locale-${page.id}`}>Lingua</Label>
              <Input
                id={`tr-locale-${page.id}`}
                required
                maxLength={2}
                minLength={2}
                value={locale}
                onChange={(e) => setLocale(e.target.value.toLowerCase())}
              />
            </FieldGroup>
            <FieldGroup>
              <Label htmlFor={`tr-slug-${page.id}`}>Slug</Label>
              <Input id={`tr-slug-${page.id}`} required value={slug} onChange={(e) => setSlug(e.target.value)} />
            </FieldGroup>
          </div>
          <FieldGroup>
            <Label htmlFor={`tr-title-${page.id}`}>Titolo</Label>
            <Input id={`tr-title-${page.id}`} required value={title} onChange={(e) => setTitle(e.target.value)} />
          </FieldGroup>
          <FieldGroup>
            <Label htmlFor={`tr-content-${page.id}`}>Contenuto (Markdown)</Label>
            <TextArea
              id={`tr-content-${page.id}`}
              required
              rows={10}
              value={content}
              onChange={(e) => setContent(e.target.value)}
            />
          </FieldGroup>
          {error && (
            <div className="mb-4">
              <Alert kind="error">{error}</Alert>
            </div>
          )}
          <div className="flex gap-3">
            <Button type="submit">Crea traduzione</Button>
            <Button type="button" variant="ghost" onClick={() => setAdding(false)}>
              Annulla
            </Button>
          </div>
        </form>
      )}
    </div>
  );
}
