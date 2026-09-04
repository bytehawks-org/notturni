"use client";

import Link from "next/link";
import { useRouter, useParams } from "next/navigation";
import { useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { RichTextEditor } from "@/components/editor/RichTextEditor";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";

const FORM_ID = "new-page-form";

/** Pagine statiche del blog (CLAUDE.md #1): niente tag, categorie o
 * pubblicazioni, editor senza il pulsante "Nota" — solo titolo/slug/lingua/
 * contenuto/pubblicata, stesso concetto delle pagine di piattaforma (ora
 * gestite dall'app admin separata, frontend/admin/, non più qui). */
export default function NewBlogPagePage() {
  const params = useParams<{ slug: string }>();
  const router = useRouter();
  const { authFetch } = useAuth();

  const [slug, setSlug] = useState("");
  const [locale, setLocale] = useState("it");
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [isPublished, setIsPublished] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      const page = await authFetch((token) =>
        api.blogs.createPage(token, params.slug, { slug, locale, title, content, is_published: isPublished })
      );
      router.push(`/dashboard/blogs/${params.slug}/pages/${page.id}`);
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
          {submitting ? "Creazione…" : "Crea pagina"}
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

        <div className="mb-8 flex items-center gap-3 text-sm text-muted">
          <span className="flex items-center gap-1">
            {params.slug}/pagina/
            <input
              value={slug}
              onChange={(e) => setSlug(e.target.value)}
              placeholder="slug-della-pagina"
              required
              className="border-0 bg-transparent p-0 text-foreground/70 placeholder:text-muted focus:text-foreground focus:outline-none"
              style={{ width: `${Math.max(slug.length, 14) + 1}ch` }}
            />
          </span>
          <span className="flex items-center gap-1">
            lingua
            <input
              value={locale}
              onChange={(e) => setLocale(e.target.value.toLowerCase())}
              maxLength={2}
              required
              className="w-8 border-0 bg-transparent p-0 text-foreground/70 focus:text-foreground focus:outline-none"
            />
          </span>
        </div>

        <RichTextEditor value={content} onChange={setContent} blogSlug={params.slug} authFetch={authFetch} />

        <div className="mt-6">
          <label className="flex items-center gap-2 text-sm text-foreground">
            <input type="checkbox" checked={isPublished} onChange={(e) => setIsPublished(e.target.checked)} />
            Pubblica subito
          </label>
        </div>

        {error && (
          <div className="mt-6">
            <Alert kind="error">{error}</Alert>
          </div>
        )}
      </form>
    </div>
  );
}
