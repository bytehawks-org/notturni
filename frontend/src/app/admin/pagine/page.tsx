"use client";

import { useTranslations } from "next-intl";
import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { RichTextEditor } from "@/components/editor/RichTextEditorLazy";
import { TranslationsBar } from "@/components/editor/TranslationsBar";
import { SearchInput } from "@/components/SearchInput";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { Page, PageTranslationSummary } from "@/lib/types";

function errorMessage(err: unknown, fallback: string): string {
  return err instanceof ApiClientError ? err.message : fallback;
}

export default function DashboardPagesPage() {
  const { accessToken } = useAuth();
  const t = useTranslations("AdminPages");
  const tc = useTranslations("Common");
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
      .catch((err) => setError(errorMessage(err, tc("unexpectedError"))));
  }, [accessToken, locale, q, tc]);

  useEffect(load, [load]);

  return (
    <div>
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <h1 className="font-serif text-2xl text-foreground">{t("title")}</h1>
        <div className="flex items-center gap-3">
          <SearchInput value={q} onChange={setQ} placeholder={t("searchPlaceholder")} />
          <Label htmlFor="locale-filter">{t("language")}</Label>
          <Input
            id="locale-filter"
            className="w-16"
            maxLength={2}
            value={locale}
            onChange={(e) => setLocale(e.target.value.toLowerCase())}
          />
          <Button onClick={() => setShowCreate((s) => !s)}>{showCreate ? tc("cancel") : t("newPage")}</Button>
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
                  /{page.slug} · {page.locale} · {page.is_published ? t("published") : t("draft")}
                </p>
              </div>
              <Button variant="secondary" onClick={() => setEditingId(editingId === page.id ? null : page.id)}>
                {editingId === page.id ? t("close") : t("edit")}
              </Button>
            </div>
            {editingId === page.id && <EditPageForm page={page} onSaved={load} />}
          </Card>
        ))}
        {pages !== null && pages.length === 0 && (
          <p className="text-sm text-muted">{t("emptyForLocale", { locale })}</p>
        )}
      </div>
    </div>
  );
}

function CreatePageForm({ defaultLocale, onCreated }: { defaultLocale: string; onCreated: () => void }) {
  const { authFetch } = useAuth();
  const t = useTranslations("AdminPages");
  const tc = useTranslations("Common");
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
      setError(errorMessage(err, tc("unexpectedError")));
    }
  }

  return (
    <Card className="mb-6">
      <CardTitle>{t("createTitle")}</CardTitle>
      <form onSubmit={handleSubmit}>
        <div className="grid gap-4 sm:grid-cols-2">
          <FieldGroup>
            <Label htmlFor="new-page-slug">{t("slug")}</Label>
            <Input id="new-page-slug" required value={slug} onChange={(e) => setSlug(e.target.value)} />
          </FieldGroup>
          <FieldGroup>
            <Label htmlFor="new-page-locale">{t("language")}</Label>
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
          <Label htmlFor="new-page-title">{t("titleField")}</Label>
          <Input id="new-page-title" required value={title} onChange={(e) => setTitle(e.target.value)} />
        </FieldGroup>
        <div className="mb-4">
          <RichTextEditor value={content} onChange={setContent} authFetch={authFetch} stickyToolbar={false} />
        </div>
        <FieldGroup>
          <label className="flex items-center gap-2 text-sm text-foreground">
            <input type="checkbox" checked={isPublished} onChange={(e) => setIsPublished(e.target.checked)} />
            {t("publishNow")}
          </label>
        </FieldGroup>
        {error && (
          <div className="mb-4">
            <Alert kind="error">{error}</Alert>
          </div>
        )}
        <Button type="submit">{t("create")}</Button>
      </form>
    </Card>
  );
}

function EditPageForm({ page, onSaved }: { page: Page; onSaved: () => void }) {
  const { authFetch } = useAuth();
  const t = useTranslations("AdminPages");
  const tc = useTranslations("Common");
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
      setError(errorMessage(err, tc("unexpectedError")));
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
          <Label htmlFor={`edit-title-${page.id}`}>{t("titleField")}</Label>
          <Input id={`edit-title-${page.id}`} value={title} onChange={(e) => setTitle(e.target.value)} />
        </FieldGroup>
        <div className="mb-4">
          <RichTextEditor value={content} onChange={setContent} authFetch={authFetch} stickyToolbar={false} />
        </div>
        <FieldGroup>
          <label className="flex items-center gap-2 text-sm text-foreground">
            <input type="checkbox" checked={isPublished} onChange={(e) => setIsPublished(e.target.checked)} />
            {t("publishedCheckbox")}
          </label>
        </FieldGroup>
        {error && (
          <div className="mb-4">
            <Alert kind="error">{error}</Alert>
          </div>
        )}
        <Button type="submit">{tc("save")}</Button>
      </form>

      <TranslationsBar
        currentId={page.id}
        currentLocale={page.locale}
        translations={translations}
        hrefFor={() => "/admin/pagine"}
        suggestedLocales={[]}
        authFetch={authFetch}
        withNotes={false}
        onAddTranslation={handleAddTranslation}
      />
    </div>
  );
}
