"use client";

import Link from "next/link";
import { useRouter, useParams } from "next/navigation";
import { useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { CoverImageUpload } from "@/components/editor/CoverImageUpload";
import { RichTextEditor } from "@/components/editor/RichTextEditor";
import { TagInput } from "@/components/editor/TagInput";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";

const FORM_ID = "new-post-form";

export default function NewPostPage() {
  const params = useParams<{ slug: string }>();
  const router = useRouter();
  const { authFetch } = useAuth();

  const [slug, setSlug] = useState("");
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [coverImageUrl, setCoverImageUrl] = useState<string | null>(null);
  const [coverImageIsSensitive, setCoverImageIsSensitive] = useState(false);
  const [tags, setTags] = useState<string[]>([]);
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
          tags,
        })
      );
      router.push(`/dashboard/blogs/${params.slug}/posts/${post.id}`);
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : "Errore imprevisto.");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="mx-auto max-w-2xl">
      <div className="mb-10 flex items-center justify-between">
        <Link href={`/dashboard/blogs/${params.slug}`} className="text-sm text-muted hover:text-foreground">
          ← Torna al blog
        </Link>
        <Button type="submit" form={FORM_ID} disabled={submitting || !content.trim()}>
          {submitting ? "Creazione…" : "Crea bozza"}
        </Button>
      </div>

      <form id={FORM_ID} onSubmit={handleSubmit}>
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Titolo"
          required
          className="mb-3 w-full border-0 bg-transparent font-serif text-5xl font-semibold leading-tight text-foreground placeholder:text-muted/70 focus:outline-none"
        />

        <div className="mb-8 flex items-center gap-1 text-sm text-muted">
          <span>{params.slug}/</span>
          <input
            value={slug}
            onChange={(e) => setSlug(e.target.value)}
            placeholder="slug-del-post"
            required
            className="border-0 bg-transparent p-0 text-foreground/70 placeholder:text-muted focus:text-foreground focus:outline-none"
            style={{ width: `${Math.max(slug.length, 14) + 1}ch` }}
          />
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
      </form>
    </div>
  );
}
