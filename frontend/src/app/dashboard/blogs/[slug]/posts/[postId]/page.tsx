"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { CategorySelect } from "@/components/editor/CategorySelect";
import { CoverImageUpload } from "@/components/editor/CoverImageUpload";
import { EditorRail } from "@/components/editor/EditorRail";
import { PostCommentsModeControl, PostCrawlingControl } from "@/components/editor/PostMetaControls";
import { PostStatusControl } from "@/components/editor/PostStatusControl";
import { PublicationSelect } from "@/components/editor/PublicationSelect";
import { RichTextEditor } from "@/components/editor/RichTextEditorLazy";
import { TagInput } from "@/components/editor/TagInput";
import { TranslationsBar } from "@/components/editor/TranslationsBar";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { SensitivityCategory } from "@/lib/content-media";
import { displayPostStatus } from "@/lib/post-status";
import { type CommentsMode, type Post, type PostNote, type PostTranslationSummary } from "@/lib/types";

const FORM_ID = "edit-post-form";

function errorMessage(err: unknown, fallback: string): string {
  return err instanceof ApiClientError ? err.message : fallback;
}

function RailLabel({ children }: { children: React.ReactNode }) {
  return (
    <span className="mb-1.5 block text-xs font-semibold uppercase tracking-[.04em] text-muted">{children}</span>
  );
}

export default function PostEditorPage() {
  const params = useParams<{ slug: string; postId: string }>();
  const router = useRouter();
  const { user, accessToken, authFetch } = useAuth();
  const t = useTranslations("PostEditorPage");
  const tc = useTranslations("Common");
  const tStatus = useTranslations("Status");

  const [post, setPost] = useState<Post | null>(null);
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [coverImageUrl, setCoverImageUrl] = useState<string | null>(null);
  const [coverImageIsSensitive, setCoverImageIsSensitive] = useState(false);
  const [coverImageCategories, setCoverImageCategories] = useState<SensitivityCategory[]>([]);
  const [coverImageAltText, setCoverImageAltText] = useState("");
  const [tags, setTags] = useState<string[]>([]);
  const [categoryId, setCategoryId] = useState<string | null>(null);
  const [publicationId, setPublicationId] = useState<string | null>(null);
  const [commentsMode, setCommentsMode] = useState<CommentsMode | null>(null);
  const [searchIndexingEnabled, setSearchIndexingEnabled] = useState<boolean | null>(null);
  const [aiCrawlingEnabled, setAiCrawlingEnabled] = useState<boolean | null>(null);
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
        setCoverImageAltText(p.cover_image_alt_text);
        setTags(p.manual_tags);
        setCategoryId(p.category?.id ?? null);
        setPublicationId(p.publication?.id ?? null);
        setCommentsMode(p.comments_mode);
        setSearchIndexingEnabled(p.search_indexing_enabled);
        setAiCrawlingEnabled(p.ai_crawling_enabled);
        setNotes(p.notes);
      })
      .catch((err) => setError(errorMessage(err, tc("unexpectedError"))));
    api.posts.translations(params.postId).then(setTranslations).catch(() => undefined);
  }, [params.postId, accessToken, tc]);

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
          cover_image_alt_text: coverImageAltText,
          tags,
          category_id: categoryId,
          publication_id: publicationId,
          comments_mode: commentsMode,
          search_indexing_enabled: searchIndexingEnabled,
          ai_crawling_enabled: aiCrawlingEnabled,
          notes,
        })
      );
      setPost(updated);
      setNotes(updated.notes);
      setSaved(true);
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    } finally {
      setSaving(false);
    }
  }

  async function handlePublish(publishedAt?: string) {
    try {
      const updated = await authFetch((token) => api.posts.publish(token, params.postId, publishedAt));
      setPost(updated);
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    }
  }

  async function handleChangeStatus(target: "draft" | "review") {
    try {
      const updated = await authFetch((token) =>
        target === "review" ? api.posts.submitForReview(token, params.postId) : api.posts.returnToDraft(token, params.postId)
      );
      setPost(updated);
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
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

  if (!post) return error ? <Alert kind="error">{error}</Alert> : <p className="text-sm text-muted">{t("loading")}</p>;

  return (
    <div className="mx-auto max-w-[1080px]">
      <div className="mb-6 flex flex-wrap items-center justify-between gap-y-2 border-b border-border pb-4">
        <div className="flex min-w-0 items-center gap-3 text-sm text-muted">
          <Link href={`/dashboard/blogs/${params.slug}`} className="hover:text-foreground">
            ‹ {params.slug}
          </Link>
          <span className="text-border">|</span>
          <span className="font-mono text-xs">
            {tStatus(displayPostStatus(post))} · {saving ? t("savingInline") : t("savedInline")}
          </span>
        </div>
        <div className="flex flex-wrap items-center gap-4">
          {post.status === "published" && (
            <Link href={post.permalink} className="text-sm text-muted hover:text-foreground">
              {t("preview")}
            </Link>
          )}
          <Button type="submit" form={FORM_ID} variant="secondary" disabled={saving}>
            {saving ? t("savingButton") : t("saveButton")}
          </Button>
        </div>
      </div>

      <div className="grid gap-8 lg:grid-cols-[minmax(0,1fr)_300px]">
        <form id={FORM_ID} onSubmit={handleSave} className="mx-auto w-full max-w-[680px]">
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            required
            className="mb-6 w-full border-0 bg-transparent font-serif text-3xl font-semibold leading-tight text-foreground placeholder:text-muted/70 focus:outline-none sm:text-[42px]"
          />

          <div className="mb-8">
            <CoverImageUpload
              value={coverImageUrl}
              isSensitive={coverImageIsSensitive}
              categories={coverImageCategories}
              altText={coverImageAltText}
              onAltTextChange={setCoverImageAltText}
              onChange={(url, sensitive, categories) => {
                setCoverImageUrl(url);
                setCoverImageIsSensitive(sensitive);
                setCoverImageCategories(categories);
              }}
              onUpload={(file) => authFetch((token) => api.blogs.uploadMedia(token, params.slug, file))}
            />
          </div>

          <RichTextEditor
            value={content}
            onChange={setContent}
            blogSlug={params.slug}
            authFetch={authFetch}
            notes={notes}
            onNotesChange={setNotes}
          />

          {error && (
            <div className="mt-6">
              <Alert kind="error">{error}</Alert>
            </div>
          )}
          {saved && (
            <div className="mt-6">
              <Alert kind="success">{t("savedAlert")}</Alert>
            </div>
          )}
        </form>

        <EditorRail
          tabs={[
            {
              id: "post",
              label: t("postTab"),
              content: (
                <>
                  <PostStatusControl post={post} onChangeStatus={handleChangeStatus} onPublish={handlePublish} />
                  <div>
                    <RailLabel>{t("slug")}</RailLabel>
                    <span className="block truncate rounded-lg border border-border bg-surface px-3 py-2.5 font-mono text-[13px] text-muted">
                      /{params.slug}/<span className="text-foreground">{post.slug}</span>
                    </span>
                  </div>
                  <CategorySelect blogSlug={params.slug} value={categoryId} onChange={setCategoryId} />
                  <PublicationSelect blogSlug={params.slug} value={publicationId} onChange={setPublicationId} />
                  <div>
                    <RailLabel>{t("tags")}</RailLabel>
                    <TagInput value={tags} onChange={setTags} />
                  </div>
                  <PostCommentsModeControl value={commentsMode} onChange={setCommentsMode} />
                  <PostCrawlingControl
                    label={t("searchEngines")}
                    value={searchIndexingEnabled}
                    onChange={setSearchIndexingEnabled}
                  />
                  <PostCrawlingControl label={t("aiCrawlers")} value={aiCrawlingEnabled} onChange={setAiCrawlingEnabled} />
                </>
              ),
            },
            {
              id: "translations",
              label: t("translationsTab"),
              badge: translations.length,
              content: (
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
              ),
            },
          ]}
        />
      </div>
    </div>
  );
}
