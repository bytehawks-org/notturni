"use client";

import { useRouter, useParams } from "next/navigation";
import { useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { CoverImageUpload } from "@/components/editor/CoverImageUpload";
import { RichTextEditor } from "@/components/editor/RichTextEditor";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";

export default function NewPostPage() {
  const params = useParams<{ slug: string }>();
  const router = useRouter();
  const { authFetch } = useAuth();

  const [slug, setSlug] = useState("");
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [coverImageUrl, setCoverImageUrl] = useState<string | null>(null);
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
    <Card className="mx-auto max-w-2xl">
      <form onSubmit={handleSubmit}>
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Titolo"
          required
          className="mb-4 w-full border-0 bg-transparent font-serif text-3xl font-semibold text-foreground placeholder:text-muted focus:outline-none"
        />

        <FieldGroup>
          <Label htmlFor="post-slug">Slug (fa parte dell&apos;URL del post)</Label>
          <Input id="post-slug" required value={slug} onChange={(e) => setSlug(e.target.value)} />
        </FieldGroup>

        <div className="mb-4">
          <CoverImageUpload
            value={coverImageUrl}
            onChange={setCoverImageUrl}
            blogSlug={params.slug}
            authFetch={authFetch}
          />
        </div>

        <div className="mb-4">
          <RichTextEditor
            value={content}
            onChange={setContent}
            blogSlug={params.slug}
            authFetch={authFetch}
          />
        </div>

        {error && (
          <div className="mb-4">
            <Alert kind="error">{error}</Alert>
          </div>
        )}
        <Button type="submit" disabled={submitting || !content.trim()}>
          {submitting ? "Creazione…" : "Crea bozza"}
        </Button>
      </form>
    </Card>
  );
}
