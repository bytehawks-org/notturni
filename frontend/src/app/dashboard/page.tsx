"use client";

import Link from "next/link";
import { useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { Blog } from "@/lib/types";

export default function DashboardHomePage() {
  const { authFetch } = useAuth();
  const [blogs, setBlogs] = useState<Blog[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [showCreate, setShowCreate] = useState(false);
  const [slug, setSlug] = useState("");
  const [title, setTitle] = useState("");
  const [createError, setCreateError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    authFetch((token) => api.blogs.listMine(token))
      .then(setBlogs)
      .catch((err) => setError(err instanceof ApiClientError ? err.message : "Errore imprevisto."));
  }, [authFetch]);

  async function handleCreate(event: FormEvent) {
    event.preventDefault();
    setCreateError(null);
    setSubmitting(true);
    try {
      const blog = await authFetch((token) => api.blogs.create(token, { slug, title }));
      setBlogs((prev) => [...(prev ?? []), blog]);
      setShowCreate(false);
      setSlug("");
      setTitle("");
    } catch (err) {
      setCreateError(err instanceof ApiClientError ? err.message : "Errore imprevisto.");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div>
      <div className="mb-6 flex items-center justify-between">
        <h1 className="font-serif text-2xl text-foreground">I miei blog</h1>
        {!showCreate && (blogs?.length ?? 0) < 5 && (
          <Button onClick={() => setShowCreate(true)}>Nuovo blog</Button>
        )}
      </div>

      {showCreate && (
        <Card className="mb-6">
          <CardTitle>Nuovo blog</CardTitle>
          <form onSubmit={handleCreate}>
            <FieldGroup>
              <Label htmlFor="slug">Slug (sottodominio)</Label>
              <Input
                id="slug"
                required
                minLength={4}
                pattern="[a-z0-9]+(-[a-z0-9]+)*"
                placeholder="il-mio-blog"
                value={slug}
                onChange={(e) => setSlug(e.target.value)}
              />
            </FieldGroup>
            <FieldGroup>
              <Label htmlFor="title">Titolo</Label>
              <Input id="title" required value={title} onChange={(e) => setTitle(e.target.value)} />
            </FieldGroup>
            {createError && (
              <div className="mb-4">
                <Alert kind="error">{createError}</Alert>
              </div>
            )}
            <div className="flex gap-2">
              <Button type="submit" disabled={submitting}>
                {submitting ? "Creazione…" : "Crea"}
              </Button>
              <Button type="button" variant="secondary" onClick={() => setShowCreate(false)}>
                Annulla
              </Button>
            </div>
          </form>
        </Card>
      )}

      {error && <Alert kind="error">{error}</Alert>}

      {blogs === null && !error && <p className="text-sm text-muted">Caricamento…</p>}

      {blogs !== null && blogs.length === 0 && (
        <p className="text-sm text-muted">Non hai ancora nessun blog.</p>
      )}

      <div className="grid gap-4 sm:grid-cols-2">
        {blogs?.map((blog) => (
          <Link key={blog.id} href={`/dashboard/blogs/${blog.slug}`}>
            <Card className="h-full transition hover:border-primary">
              <h2 className="font-serif text-lg text-foreground">{blog.title}</h2>
              <p className="mt-1 text-sm text-muted">{blog.slug}.notturni.eu</p>
            </Card>
          </Link>
        ))}
      </div>
    </div>
  );
}
