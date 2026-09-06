"use client";

import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import {
  BLOG_VISIBILITY_LABELS,
  type Blog,
  type BlogVisibility,
  type Category,
} from "@/lib/types";

import { errorMessage } from "./shared";

export function SettingsTab({
  blog,
  canEdit,
  onUpdated,
}: {
  blog: Blog;
  canEdit: boolean;
  onUpdated: (blog: Blog) => void;
}) {
  const { authFetch } = useAuth();
  const [title, setTitle] = useState(blog.title);
  const [subtitle, setSubtitle] = useState(blog.subtitle ?? "");
  const [description, setDescription] = useState(blog.description ?? "");
  const [visibility, setVisibility] = useState<BlogVisibility>(blog.visibility);
  const [allowAnonymous, setAllowAnonymous] = useState(blog.allow_anonymous_comments);
  const [mentionsEnabled, setMentionsEnabled] = useState(blog.mentions_enabled);
  const [staticPagesEnabled, setStaticPagesEnabled] = useState(blog.static_pages_enabled);
  const [defaultAuthorName, setDefaultAuthorName] = useState(blog.default_author_display_name ?? "");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  async function handleSave() {
    setSaving(true);
    setError(null);
    try {
      const updated = await authFetch((token) =>
        api.blogs.update(token, blog.slug, {
          title,
          subtitle,
          description,
          visibility,
          allow_anonymous_comments: allowAnonymous,
          mentions_enabled: mentionsEnabled,
          static_pages_enabled: staticPagesEnabled,
          default_author_display_name: defaultAuthorName,
        })
      );
      onUpdated(updated);
      setSaved(true);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <>
      <Card>
        <FieldGroup>
          <Label htmlFor="blog-title">Titolo</Label>
          <Input id="blog-title" value={title} onChange={(e) => setTitle(e.target.value)} disabled={!canEdit} />
        </FieldGroup>
        <FieldGroup>
          <Label htmlFor="blog-subtitle">Sottotitolo (max 64)</Label>
          <Input
            id="blog-subtitle"
            maxLength={64}
            value={subtitle}
            onChange={(e) => setSubtitle(e.target.value)}
            disabled={!canEdit}
          />
        </FieldGroup>
        <FieldGroup>
          <Label htmlFor="blog-description">Descrizione breve (max 256)</Label>
          <textarea
            id="blog-description"
            maxLength={256}
            rows={3}
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            disabled={!canEdit}
            className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground disabled:opacity-60"
          />
          <p className="mt-1 text-xs text-muted">{description.length}/256</p>
        </FieldGroup>
        <FieldGroup>
          <Label htmlFor="blog-visibility">Visibilità</Label>
          <select
            id="blog-visibility"
            value={visibility}
            onChange={(e) => setVisibility(e.target.value as BlogVisibility)}
            disabled={!canEdit}
            className="w-full max-w-xs rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground disabled:opacity-60"
          >
            {(Object.keys(BLOG_VISIBILITY_LABELS) as BlogVisibility[]).map((v) => (
              <option key={v} value={v}>
                {BLOG_VISIBILITY_LABELS[v]}
              </option>
            ))}
          </select>
          <p className="mt-1 text-xs text-muted">
            <strong>Pubblico</strong>: visibile a tutti e nel feed della homepage.{" "}
            <strong>Solo iscritti</strong>: leggibile solo da chi ha un account.{" "}
            <strong>Privato</strong>: diario visibile e scrivibile solo da te.
          </p>
        </FieldGroup>
        <FieldGroup>
          <Label htmlFor="blog-pen-name">Nome pubblico predefinito per gli autori</Label>
          <Input
            id="blog-pen-name"
            placeholder="es. La redazione — lasciare vuoto per usare lo username"
            value={defaultAuthorName}
            onChange={(e) => setDefaultAuthorName(e.target.value)}
            disabled={!canEdit}
          />
          <p className="mt-1 text-xs text-muted">
            Nome pubblico degli autori sui post di questo blog. Se impostato, vale sempre e non
            è sovrascrivibile dal singolo autore (todo/USERS.md #2).
          </p>
        </FieldGroup>
        <FieldGroup>
          <label className="flex items-center gap-2 text-sm text-foreground">
            <input
              type="checkbox"
              checked={allowAnonymous}
              onChange={(e) => setAllowAnonymous(e.target.checked)}
              disabled={!canEdit}
            />
            Consenti commenti da chi non è registrato (con moderazione obbligatoria)
          </label>
        </FieldGroup>
        <FieldGroup>
          <label className="flex items-center gap-2 text-sm text-foreground">
            <input
              type="checkbox"
              checked={mentionsEnabled}
              onChange={(e) => setMentionsEnabled(e.target.checked)}
              disabled={!canEdit}
            />
            Trasforma le @menzioni nei post in link al profilo dell&apos;utente citato
          </label>
        </FieldGroup>
        <FieldGroup>
          <label className="flex items-center gap-2 text-sm text-foreground">
            <input
              type="checkbox"
              checked={staticPagesEnabled}
              onChange={(e) => setStaticPagesEnabled(e.target.checked)}
              disabled={!canEdit}
            />
            Pagine statiche (Chi sono, Contattami, ...) — disattiva di default
          </label>
        </FieldGroup>
        <p className="mb-4 text-sm text-muted">Lingua di default: {blog.default_locale}</p>
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
        {canEdit && (
          <Button onClick={handleSave} disabled={saving}>
            {saving ? "Salvataggio…" : "Salva impostazioni"}
          </Button>
        )}
      </Card>

      <div className="mt-6">
        <CategoriesSettings blogSlug={blog.slug} canEdit={canEdit} />
      </div>
    </>
  );
}

function CategoriesSettings({ blogSlug, canEdit }: { blogSlug: string; canEdit: boolean }) {
  const { authFetch } = useAuth();
  const [categories, setCategories] = useState<Category[] | null>(null);
  const [name, setName] = useState("");
  const [slug, setSlug] = useState("");
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    api.blogs.listCategories(blogSlug).then(setCategories).catch(() => undefined);
  }, [blogSlug]);

  useEffect(load, [load]);

  async function handleAdd(event: FormEvent) {
    event.preventDefault();
    setError(null);
    try {
      await authFetch((token) => api.blogs.createCategory(token, blogSlug, { name, slug }));
      setName("");
      setSlug("");
      load();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  async function handleDelete(categoryId: string) {
    try {
      await authFetch((token) => api.blogs.deleteCategory(token, blogSlug, categoryId));
      load();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  return (
    <Card>
      <CardTitle>Categorie</CardTitle>
      <p className="mb-4 text-sm text-muted">
        Tassonomia del blog: a differenza dei tag, un post ne ha al più una — pensata per una
        classificazione più organica dei contenuti.
      </p>
      <ul className="mb-4 space-y-2">
        {categories?.map((c) => (
          <li key={c.id} className="flex items-center justify-between text-sm">
            <span className="text-foreground">{c.name}</span>
            {canEdit && (
              <button
                type="button"
                onClick={() => handleDelete(c.id)}
                className="text-muted hover:text-foreground"
              >
                Rimuovi
              </button>
            )}
          </li>
        ))}
        {categories?.length === 0 && <p className="text-sm text-muted">Nessuna categoria.</p>}
      </ul>
      {canEdit && (
        <form onSubmit={handleAdd} className="flex flex-wrap items-end gap-3">
          <div>
            <Label htmlFor="cat-name">Nome</Label>
            <Input id="cat-name" required value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <div>
            <Label htmlFor="cat-slug">Slug</Label>
            <Input id="cat-slug" required value={slug} onChange={(e) => setSlug(e.target.value)} />
          </div>
          <Button type="submit">Aggiungi</Button>
        </form>
      )}
      {error && (
        <div className="mt-3">
          <Alert kind="error">{error}</Alert>
        </div>
      )}
    </Card>
  );
}
