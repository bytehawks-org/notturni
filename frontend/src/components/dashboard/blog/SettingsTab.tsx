"use client";

import { useLocale, useTranslations } from "next-intl";
import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle, SectionLabel } from "@/components/ui/Card";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { Toggle } from "@/components/ui/Controls";
import { FieldGroup, Input, Label, TextArea } from "@/components/ui/Field";
import { useToast } from "@/components/ui/Toast";
import { CoverImageUpload } from "@/components/editor/CoverImageUpload";
import { FaviconUpload } from "@/components/dashboard/blog/FaviconUpload";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import { languageName } from "@/lib/languages";
import { SITE_HOST } from "@/lib/site";
import type { Blog, BlogMember, BlogVisibility, Category, CommentsMode } from "@/lib/types";

import { errorMessage } from "./shared";

const SECTIONS = ["identity", "visibility", "languages", "features", "domain", "danger"] as const;
const VISIBILITIES: BlogVisibility[] = ["public", "members", "private"];
const COMMENT_MODES: CommentsMode[] = ["members", "everyone", "closed"];

/** Tab Impostazioni (mockup 5c): identità, visibilità, lingue, funzionalità,
 * dominio (disattivato finché non arriva il routing per sottodominio) e
 * zona pericolosa — trasferimento a un coautore, pausa, export ZIP,
 * cancellazione con 30 giorni di tolleranza e ripristino. */
export function SettingsTab({ blog, canEdit, onUpdated }: { blog: Blog; canEdit: boolean; onUpdated: (blog: Blog) => void }) {
  const { user, authFetch } = useAuth();
  const t = useTranslations("Settings");
  const tc = useTranslations("Common");
  const locale = useLocale();
  const notify = useToast();

  const [title, setTitle] = useState(blog.title);
  const [subtitle, setSubtitle] = useState(blog.subtitle ?? "");
  const [description, setDescription] = useState(blog.description ?? "");
  const [visibility, setVisibility] = useState<BlogVisibility>(blog.visibility);
  const [commentsMode, setCommentsMode] = useState<CommentsMode>(blog.comments_mode);
  const [mentionsEnabled, setMentionsEnabled] = useState(blog.mentions_enabled);
  const [staticPagesEnabled, setStaticPagesEnabled] = useState(blog.static_pages_enabled);
  const [searchIndexingEnabled, setSearchIndexingEnabled] = useState(blog.search_indexing_enabled);
  const [aiCrawlingEnabled, setAiCrawlingEnabled] = useState(blog.ai_crawling_enabled);
  const [defaultAuthorName, setDefaultAuthorName] = useState(blog.default_author_display_name ?? "");
  const [extraLocales, setExtraLocales] = useState<string[]>(blog.extra_locales ?? []);
  const [profileLanguages, setProfileLanguages] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const [members, setMembers] = useState<BlogMember[]>([]);
  const [transferTo, setTransferTo] = useState("");
  const [showTransfer, setShowTransfer] = useState(false);
  const [showDelete, setShowDelete] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    if (!user) return;
    api.users
      .profile(user.username)
      .then((p) => setProfileLanguages([p.native_language, ...p.fallback_languages].filter((l): l is string => !!l)))
      .catch(() => undefined);
    if (canEdit) {
      authFetch((token) => api.blogs.members(token, blog.slug))
        .then(setMembers)
        .catch(() => undefined);
    }
  }, [user, canEdit, authFetch, blog.slug]);

  const run = useCallback(
    async (key: string, fn: () => Promise<Blog | void>, done?: string) => {
      setBusy(key);
      setError(null);
      try {
        const updated = await fn();
        if (updated) onUpdated(updated);
        if (done) notify(done);
      } catch (err) {
        setError(errorMessage(err));
      } finally {
        setBusy(null);
      }
    },
    [notify, onUpdated]
  );

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
          comments_mode: commentsMode,
          mentions_enabled: mentionsEnabled,
          static_pages_enabled: staticPagesEnabled,
          search_indexing_enabled: searchIndexingEnabled,
          ai_crawling_enabled: aiCrawlingEnabled,
          default_author_display_name: defaultAuthorName,
          extra_locales: extraLocales,
        })
      );
      onUpdated(updated);
      notify(t("savedToast"));
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSaving(false);
    }
  }

  async function handleExport() {
    setBusy("export");
    try {
      const blob = await authFetch(async (token) => {
        const res = await fetch(api.blogs.exportUrl(blog.slug), { headers: { Authorization: `Bearer ${token}` } });
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        return res.blob();
      });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `notturni-${blog.slug}.zip`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(null);
    }
  }

  const coauthors = members.filter((m) => m.role === "co_autore");
  const addableLanguages = profileLanguages.filter((l) => l !== blog.default_locale && !extraLocales.includes(l));
  const fieldCls = "w-full rounded-lg border border-border bg-surface px-3 py-2.5 text-sm text-foreground focus:outline-none focus:border-primary focus:ring-[3px] focus:ring-primary/20 disabled:opacity-60";

  return (
    <div className="grid gap-8 lg:grid-cols-[170px_minmax(0,1fr)]">
      <nav className="hidden flex-col gap-0.5 text-sm text-muted lg:sticky lg:top-6 lg:flex lg:h-fit">
        {SECTIONS.map((id) => (
          <a key={id} href={`#settings-${id}`} className={`rounded-md px-2.5 py-1.5 no-underline hover:text-foreground ${id === "danger" ? "text-danger" : ""}`}>
            {t(`sections.${id}`)}
          </a>
        ))}
      </nav>

      <div className="flex flex-col gap-10">
        {blog.deleted_at && (
          <Alert kind="error">
            {t("deletedNotice", { date: formatDate(blog.deleted_at, locale) })}{" "}
            <button type="button" className="font-semibold underline" onClick={() => run("restore", () => authFetch((tk) => api.blogs.restore(tk, blog.slug)), t("restoredToast"))}>
              {t("danger.restoreAction")}
            </button>
          </Alert>
        )}
        {blog.is_paused && !blog.deleted_at && <Alert kind="info">{t("pausedNotice")}</Alert>}
        {error && <Alert kind="error">{error}</Alert>}

        <section id="settings-identity" className="flex scroll-mt-6 flex-col gap-4">
          <CardTitle>{t("sections.identity")}</CardTitle>
          <div className="grid gap-4 sm:grid-cols-2">
            <FieldGroup className="mb-0">
              <Label htmlFor="blog-title">{t("title")}</Label>
              <Input id="blog-title" value={title} onChange={(e) => setTitle(e.target.value)} disabled={!canEdit} />
            </FieldGroup>
            <FieldGroup className="mb-0">
              <Label htmlFor="blog-slug" hint={t("slugHint")}>
                {t("slug")}
              </Label>
              <span className="flex items-center gap-1 rounded-lg border border-border bg-surface px-3 py-2.5 font-mono text-[13px] text-muted">
                <span className="text-foreground">{blog.slug}</span>.{SITE_HOST}
              </span>
            </FieldGroup>
            <FieldGroup className="mb-0">
              <Label htmlFor="blog-subtitle" hint={`${subtitle.length} / 64`}>
                {t("subtitle")}
              </Label>
              <Input id="blog-subtitle" maxLength={64} value={subtitle} onChange={(e) => setSubtitle(e.target.value)} disabled={!canEdit} />
            </FieldGroup>
            <FieldGroup className="mb-0">
              <Label htmlFor="blog-pen-name">{t("defaultAuthor")}</Label>
              <Input id="blog-pen-name" placeholder={t("defaultAuthorPlaceholder")} value={defaultAuthorName} onChange={(e) => setDefaultAuthorName(e.target.value)} disabled={!canEdit} />
            </FieldGroup>
          </div>
          <FieldGroup className="mb-0">
            <Label htmlFor="blog-description" hint={`${description.length} / 256`}>
              {t("description")}
            </Label>
            <TextArea id="blog-description" maxLength={256} rows={3} value={description} onChange={(e) => setDescription(e.target.value)} disabled={!canEdit} />
          </FieldGroup>
          <div className="grid gap-4 sm:grid-cols-2">
            <FieldGroup className="mb-0">
              <Label htmlFor="blog-cover" hint={t("coverImageHint")}>
                {t("coverImage")}
              </Label>
              <div className="max-w-xs">
                <CoverImageUpload
                  value={blog.cover_image_url}
                  isSensitive={blog.cover_image_is_sensitive}
                  categories={blog.cover_image_categories}
                  onChange={(url, sensitive, categories) => {
                    if (url === null) {
                      void authFetch((tk) => api.blogs.deleteCoverImage(tk, blog.slug)).then(onUpdated);
                    } else if (url === blog.cover_image_url) {
                      void authFetch((tk) => api.blogs.updateCoverImageCategories(tk, blog.slug, categories)).then(onUpdated);
                    }
                  }}
                  onUpload={(file) =>
                    authFetch((tk) => api.blogs.uploadCoverImage(tk, blog.slug, file)).then((updated) => {
                      onUpdated(updated);
                      return { url: updated.cover_image_url ?? "", is_sensitive: updated.cover_image_is_sensitive };
                    })
                  }
                />
              </div>
            </FieldGroup>
            <FieldGroup className="mb-0">
              <Label htmlFor="blog-favicon" hint={t("faviconHint")}>
                {t("favicon")}
              </Label>
              <FaviconUpload
                value={blog.favicon_url}
                onUpload={(file) =>
                  authFetch((tk) => api.blogs.uploadFavicon(tk, blog.slug, file)).then((updated) => {
                    onUpdated(updated);
                    return updated.favicon_url;
                  })
                }
                onRemove={() => authFetch((tk) => api.blogs.deleteFavicon(tk, blog.slug)).then(onUpdated)}
              />
            </FieldGroup>
          </div>
        </section>

        <section id="settings-visibility" className="flex scroll-mt-6 flex-col gap-4">
          <CardTitle>{t("sections.visibility")}</CardTitle>
          <div className="grid gap-2.5 sm:grid-cols-3">
            {VISIBILITIES.map((v) => (
              <button
                key={v}
                type="button"
                disabled={!canEdit}
                onClick={() => setVisibility(v)}
                className={`flex flex-col gap-0.5 rounded-lg border px-3.5 py-3 text-left transition ${visibility === v ? "border-primary bg-primary/5" : "border-border hover:border-primary/40"}`}
              >
                <span className="text-sm font-semibold text-foreground">{t(`vis.${v}`)}</span>
                <span className="text-[13px] text-muted">{t(`vis.${v}Sub`)}</span>
              </button>
            ))}
          </div>
          <div className="flex flex-col gap-3 rounded-xl border border-border bg-surface px-4 py-3">
            <Toggle checked={searchIndexingEnabled} onChange={(v) => canEdit && setSearchIndexingEnabled(v)} label={t("indexing")} />
            <span className="-mt-2 text-[13px] text-muted">{t("indexingSub")}</span>
            <Toggle checked={aiCrawlingEnabled} onChange={(v) => canEdit && setAiCrawlingEnabled(v)} label={t("aiCrawling")} />
            <span className="-mt-2 text-[13px] text-muted">{t("aiCrawlingSub")}</span>
          </div>
        </section>

        <section id="settings-languages" className="flex scroll-mt-6 flex-col gap-4">
          <CardTitle>{t("sections.languages")}</CardTitle>
          <div className="flex flex-wrap items-center gap-2">
            <span className="rounded-full bg-primary/10 px-3 py-1 text-[13px] text-primary">{t("primary", { lang: languageName(blog.default_locale, locale) })}</span>
            {extraLocales.map((code) => (
              <span key={code} className="flex items-center gap-1.5 rounded-full border border-border px-3 py-1 text-[13px]">
                {languageName(code, locale)}
                {canEdit && (
                  <button type="button" onClick={() => setExtraLocales((prev) => prev.filter((c) => c !== code))} className="text-muted hover:text-foreground" aria-label={tc("remove")}>
                    ×
                  </button>
                )}
              </span>
            ))}
            {canEdit && addableLanguages.length > 0 && (
              <select
                value=""
                onChange={(e) => e.target.value && setExtraLocales((prev) => [...prev, e.target.value])}
                className="rounded-full border border-dashed border-border bg-transparent px-3 py-1 text-[13px] text-muted"
              >
                <option value="">{t("addLanguage")}</option>
                {addableLanguages.map((code) => (
                  <option key={code} value={code}>
                    {languageName(code, locale)}
                  </option>
                ))}
              </select>
            )}
          </div>
          <span className="text-[13px] text-muted">{t("languagesNote")}</span>
        </section>

        <section id="settings-features" className="flex scroll-mt-6 flex-col gap-4">
          <CardTitle>{t("sections.features")}</CardTitle>
          <div className="flex flex-col gap-3 rounded-xl border border-border bg-surface px-4 py-3">
            <Toggle checked={mentionsEnabled} onChange={(v) => canEdit && setMentionsEnabled(v)} label={t("features.mentions")} />
            <span className="-mt-2 text-[13px] text-muted">{t("features.mentionsSub")}</span>
            <Toggle checked={staticPagesEnabled} onChange={(v) => canEdit && setStaticPagesEnabled(v)} label={t("features.pages")} />
            <span className="-mt-2 text-[13px] text-muted">{t("features.pagesSub")}</span>
          </div>
          <div className="flex flex-col gap-2">
            <SectionLabel>{t("commentsPolicy")}</SectionLabel>
            <div className="grid gap-2.5 sm:grid-cols-3">
              {COMMENT_MODES.map((m) => (
                <button
                  key={m}
                  type="button"
                  disabled={!canEdit}
                  onClick={() => setCommentsMode(m)}
                  className={`flex flex-col gap-0.5 rounded-lg border px-3.5 py-3 text-left transition ${commentsMode === m ? "border-primary bg-primary/5" : "border-border hover:border-primary/40"}`}
                >
                  <span className="text-sm font-semibold text-foreground">{t(`comments.${m}`)}</span>
                  <span className="text-[13px] text-muted">{t(`comments.${m}Sub`)}</span>
                </button>
              ))}
            </div>
          </div>
        </section>

        <section id="settings-domain" className="flex scroll-mt-6 flex-col gap-3 opacity-70">
          <CardTitle>{t("sections.domain")}</CardTitle>
          <FieldGroup className="mb-0 max-w-md">
            <Label htmlFor="blog-domain">{t("customDomain")}</Label>
            <Input id="blog-domain" disabled value={blog.custom_domain ?? ""} placeholder={`${blog.slug}.example.eu`} />
          </FieldGroup>
          <span className="text-[13px] text-muted">{t("domainNote", { host: `${blog.slug}.${SITE_HOST}` })}</span>
        </section>

        {canEdit && (
          <div className="sticky bottom-4 flex justify-end lg:static">
            <Button onClick={handleSave} disabled={saving}>
              {saving ? tc("saving") : t("save")}
            </Button>
          </div>
        )}

        <CategoriesSettings blogSlug={blog.slug} canEdit={canEdit} />

        {canEdit && (
          <section id="settings-danger" className="flex scroll-mt-6 flex-col gap-3">
            <CardTitle>
              <span className="text-danger">{t("dangerZone")}</span>
            </CardTitle>
            <div className="overflow-hidden rounded-xl border border-danger/40">
              <DangerRow title={t("danger.transfer")} sub={t("danger.transferSub")}>
                {!showTransfer ? (
                  <Button size="sm" variant="secondary" disabled={coauthors.length === 0} onClick={() => setShowTransfer(true)}>
                    {t("danger.transferAction")}
                  </Button>
                ) : (
                  <form
                    className="flex items-center gap-2"
                    onSubmit={(e: FormEvent) => {
                      e.preventDefault();
                      void run("transfer", () => authFetch((tk) => api.blogs.transfer(tk, blog.slug, transferTo)), t("transferredToast")).then(() => setShowTransfer(false));
                    }}
                  >
                    <select value={transferTo} onChange={(e) => setTransferTo(e.target.value)} required className={fieldCls}>
                      <option value="">—</option>
                      {coauthors.map((m) => (
                        <option key={m.user_id} value={m.username}>
                          @{m.username}
                        </option>
                      ))}
                    </select>
                    <Button size="sm" variant="danger" type="submit" disabled={!transferTo || busy === "transfer"}>
                      {tc("confirm")}
                    </Button>
                    <Button size="sm" variant="ghost" type="button" onClick={() => setShowTransfer(false)}>
                      {tc("cancel")}
                    </Button>
                  </form>
                )}
              </DangerRow>
              <DangerRow title={blog.is_paused ? t("danger.resume") : t("danger.pause")} sub={t("danger.pauseSub")}>
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={busy === "pause"}
                  onClick={() => run("pause", () => authFetch((tk) => api.blogs.update(tk, blog.slug, { is_paused: !blog.is_paused })), blog.is_paused ? t("resumedToast") : t("pausedToast"))}
                >
                  {blog.is_paused ? t("danger.resumeAction") : t("danger.pauseAction")}
                </Button>
              </DangerRow>
              <DangerRow title={t("danger.export")} sub={t("danger.exportSub")}>
                <Button size="sm" variant="secondary" disabled={busy === "export"} onClick={handleExport}>
                  {busy === "export" ? tc("loading") : t("danger.exportAction")}
                </Button>
              </DangerRow>
              <DangerRow title={t("danger.delete")} sub={t("danger.deleteSub")}>
                {blog.deleted_at ? (
                  <Button size="sm" variant="secondary" disabled={busy === "restore"} onClick={() => run("restore", () => authFetch((tk) => api.blogs.restore(tk, blog.slug)), t("restoredToast"))}>
                    {t("danger.restoreAction")}
                  </Button>
                ) : (
                  <Button size="sm" variant="danger" onClick={() => setShowDelete(true)}>
                    {t("danger.deleteAction")}
                  </Button>
                )}
              </DangerRow>
            </div>
            <ConfirmDialog
              open={showDelete}
              title={t("confirmTitle", { title: blog.title })}
              body={t("confirmBody")}
              confirmText={blog.slug}
              confirmLabel={t("confirmLabel")}
              onCancel={() => setShowDelete(false)}
              onConfirm={() => {
                setShowDelete(false);
                void run("delete", () => authFetch((tk) => api.blogs.softDelete(tk, blog.slug, blog.slug)), t("deletedToast"));
              }}
            />
          </section>
        )}
      </div>
    </div>
  );
}

function DangerRow({ title, sub, children }: { title: string; sub: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-3 border-b border-danger/20 px-4 py-4 last:border-0 sm:flex-row sm:items-center sm:justify-between">
      <div className="flex flex-col gap-0.5">
        <span className="text-sm font-medium text-foreground">{title}</span>
        <span className="text-[13px] text-muted">{sub}</span>
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function CategoriesSettings({ blogSlug, canEdit }: { blogSlug: string; canEdit: boolean }) {
  const { authFetch } = useAuth();
  const t = useTranslations("Settings");
  const tc = useTranslations("Common");
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
    <Card className="flex flex-col gap-3">
      <CardTitle>{t("categories")}</CardTitle>
      <p className="text-[13px] text-muted">{t("categoriesSub")}</p>
      <ul className="flex flex-wrap gap-2">
        {categories?.map((c) => (
          <li key={c.id} className="flex items-center gap-1.5 rounded-full border border-border px-3 py-1 text-[13px]">
            {c.name}
            {canEdit && (
              <button type="button" onClick={() => handleDelete(c.id)} className="text-muted hover:text-foreground" aria-label={tc("remove")}>
                ×
              </button>
            )}
          </li>
        ))}
        {categories?.length === 0 && <li className="text-[13px] text-muted">{t("noCategories")}</li>}
      </ul>
      {canEdit && (
        <form onSubmit={handleAdd} className="flex flex-wrap items-end gap-3">
          <div>
            <Label htmlFor="cat-name">{t("categoryName")}</Label>
            <Input id="cat-name" required value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <div>
            <Label htmlFor="cat-slug">{t("categorySlug")}</Label>
            <Input id="cat-slug" required value={slug} onChange={(e) => setSlug(e.target.value)} />
          </div>
          <Button type="submit" size="sm">
            {tc("add")}
          </Button>
        </form>
      )}
      {error && <Alert kind="error">{error}</Alert>}
    </Card>
  );
}
