"use client";

import { useRouter, useParams } from "next/navigation";
import { useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label, TextArea } from "@/components/ui/Field";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";

export default function NewPostPage() {
  const params = useParams<{ slug: string }>();
  const router = useRouter();
  const { authFetch } = useAuth();

  const [slug, setSlug] = useState("");
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      const post = await authFetch((token) =>
        api.posts.create(token, params.slug, { slug, title, content })
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
      <CardTitle>Nuovo post</CardTitle>
      <form onSubmit={handleSubmit}>
        <FieldGroup>
          <Label htmlFor="post-title">Titolo</Label>
          <Input id="post-title" required value={title} onChange={(e) => setTitle(e.target.value)} />
        </FieldGroup>
        <FieldGroup>
          <Label htmlFor="post-slug">Slug</Label>
          <Input id="post-slug" required value={slug} onChange={(e) => setSlug(e.target.value)} />
        </FieldGroup>
        <FieldGroup>
          <Label htmlFor="post-content">Contenuto</Label>
          <TextArea
            id="post-content"
            required
            className="min-h-64"
            value={content}
            onChange={(e) => setContent(e.target.value)}
          />
        </FieldGroup>
        {error && (
          <div className="mb-4">
            <Alert kind="error">{error}</Alert>
          </div>
        )}
        <Button type="submit" disabled={submitting}>
          {submitting ? "Creazione…" : "Crea bozza"}
        </Button>
      </form>
    </Card>
  );
}
