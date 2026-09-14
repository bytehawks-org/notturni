"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useRouter, useParams } from "next/navigation";
import { useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { CategorySelect } from "@/components/editor/CategorySelect";
import { CoverImageUpload } from "@/components/editor/CoverImageUpload";
import { EditorRail } from "@/components/editor/EditorRail";
import { PublicationSelect } from "@/components/editor/PublicationSelect";
import { RichTextEditor } from "@/components/editor/RichTextEditor";
import { TagInput } from "@/components/editor/TagInput";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { SensitivityCategory } from "@/lib/content-media";
import type { PostNote } from "@/lib/types";

const FORM_ID = "new-post-form";

export default function NewPostPage() {
  const params = useParams<{ slug: string }>();
  const router = useRouter();
  const { authFetch } = useAuth();
  const t = useTranslations("NewPostPage");
  const tc = useTranslations("Common");

  const [slug, setSlug] = useState("");
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [coverImageUrl, setCoverImageUrl] = useState<string | null>(null);
  const [coverImageIsSensitive, setCoverImageIsSensitive] = useState(false);
  const [coverImageCategories, setCoverImageCategories] = useState<SensitivityCategory[]>([]);
  const [tags, setTags] = useState<string[]>([]);
  const [categoryId, setCategoryId] = useState<string | null>(null);
  const [publicationId, setPublicationId] = useState<string | null>(null);
  const [notes, setNotes] = useState<PostNote[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      const post = await authFetch((token) =>
        api.posts.create(token, params.slug, {
          slug,
          title,
          content,
          cover_image_url: coverImageUrl,
          cover_image_is_sensitive: coverImageIsSensitive,
          cover_image_categories: coverImageCategories,
          tags,
          category_id: categoryId,
          publication_id: publicationId,
          notes,
        })
      );
      router.push(`/dashboard/blogs/${params.slug}/posts/${post.id}`);
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : tc("unexpectedError"));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="mx-auto max-w-[1080px]">
      <div className="mb-6 flex flex-wrap items-center justify-between gap-y-2 border-b border-border pb-4">
        <Link href={`/dashboard/blogs/${params.slug}`} className="text-sm text-muted hover:text-foreground">
          ‹ {params.slug}
        </Link>
        <Button type="submit" form={FORM_ID} disabled={submitting || !content.trim()}>
          {submitting ? t("creating") : t("createDraft")}
        </Button>
      </div>

      <div className="grid gap-8 lg:grid-cols-[minmax(0,1fr)_300px]">
        <form id={FORM_ID} onSubmit={handleSubmit} className="mx-auto w-full max-w-[680px]">
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder={t("titlePlaceholder")}
            required
            className="mb-3 w-full border-0 bg-transparent font-serif text-3xl font-semibold leading-tight text-foreground placeholder:text-muted/70 focus:outline-none sm:text-[42px]"
          />

          <div className="mb-8 flex flex-wrap items-center gap-1 text-sm text-muted">
            <span>{params.slug}/</span>
            <input
              value={slug}
              onChange={(e) => setSlug(e.target.value)}
              placeholder={t("slugPlaceholder")}
              required
              className="border-0 bg-transparent p-0 text-foreground/70 placeholder:text-muted focus:text-foreground focus:outline-none"
              style={{ width: `${Math.max(slug.length, 14) + 1}ch` }}
            />
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
        </form>

        <EditorRail
          tabs={[
            {
              id: "post",
              label: t("postTab"),
              content: (
                <>
                  <CategorySelect blogSlug={params.slug} value={categoryId} onChange={setCategoryId} />
                  <PublicationSelect blogSlug={params.slug} value={publicationId} onChange={setPublicationId} />
                  <div>
                    <span className="mb-1.5 block text-xs font-semibold uppercase tracking-[.04em] text-muted">
                      {t("tags")}
                    </span>
                    <TagInput value={tags} onChange={setTags} />
                  </div>
                </>
              ),
            },
          ]}
        />
      </div>
    </div>
  );
}
