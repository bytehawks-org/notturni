"use client";

import Link from "next/link";
import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import {
  BLOG_VISIBILITY_LABELS,
  type Blog,
  type BlogInvitation,
  type BlogVisibility,
  type MembershipBlog,
} from "@/lib/types";

/** todo/BLOG.md #2: banda colorata sul lato destro della card in elenco. */
function VisibilityBand({ visibility }: { visibility: BlogVisibility }) {
  const style: React.CSSProperties =
    visibility === "public"
      ? { background: "#3f9142" }
      : visibility === "members"
        ? { background: "#e08a1e" }
        : { background: "repeating-linear-gradient(45deg, #1a1a1a 0 6px, #d1332f 6px 12px)" };
  return (
    <span
      aria-hidden
      className="absolute inset-y-0 right-0 w-2"
      style={style}
      title={BLOG_VISIBILITY_LABELS[visibility]}
    />
  );
}

const ROLE_LABELS: Record<string, string> = {
  autore: "Autore",
  co_autore: "Co-autore",
  revisore: "Revisore",
  mediatore: "Mediatore",
};

export default function DashboardHomePage() {
  const { user, authFetch } = useAuth();
  const [blogs, setBlogs] = useState<Blog[] | null>(null);
  const [shared, setShared] = useState<MembershipBlog[]>([]);
  const [invitations, setInvitations] = useState<BlogInvitation[]>([]);
  const [error, setError] = useState<string | null>(null);

  const [showCreate, setShowCreate] = useState(false);
  const [slug, setSlug] = useState("");
  const [title, setTitle] = useState("");
  const [subtitle, setSubtitle] = useState("");
  const [visibility, setVisibility] = useState<BlogVisibility>("public");
  // CLAUDE.md #4: suggerito come lo username di chi crea il blog finché
  // l'utente non lo tocca, resta modificabile — se lasciato vuoto il nome
  // pubblico ricade comunque sullo username (vedi _resolve_author_display_name
  // lato backend), quindi il suggerimento qui è solo per renderlo esplicito.
  const [defaultAuthorNameInput, setDefaultAuthorNameInput] = useState<string | null>(null);
  const defaultAuthorName = defaultAuthorNameInput ?? user?.username ?? "";
  const [createError, setCreateError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const load = useCallback(() => {
    authFetch((token) => api.blogs.listMine(token))
      .then(setBlogs)
      .catch((err) => setError(err instanceof ApiClientError ? err.message : "Errore imprevisto."));
    authFetch((token) => api.blogs.memberOf(token))
      .then(setShared)
      .catch(() => undefined);
    authFetch((token) => api.blogs.receivedInvitations(token))
      .then(setInvitations)
      .catch(() => undefined);
  }, [authFetch]);

  useEffect(load, [load]);

  async function handleCreate(event: FormEvent) {
    event.preventDefault();
    setCreateError(null);
    setSubmitting(true);
    try {
      const blog = await authFetch((token) =>
        api.blogs.create(token, {
          slug,
          title,
          subtitle: subtitle || null,
          visibility,
          default_author_display_name: defaultAuthorName || null,
        })
      );
      setBlogs((prev) => [...(prev ?? []), blog]);
      setShowCreate(false);
      setSlug("");
      setTitle("");
      setSubtitle("");
      setVisibility("public");
      setDefaultAuthorNameInput(null);
    } catch (err) {
      setCreateError(err instanceof ApiClientError ? err.message : "Errore imprevisto.");
    } finally {
      setSubmitting(false);
    }
  }

  async function respondInvitation(id: string, action: "accept" | "decline") {
    try {
      await authFetch((token) =>
        action === "accept"
          ? api.blogs.acceptInvitation(token, id)
          : api.blogs.declineInvitation(token, id)
      );
      load();
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : "Errore imprevisto.");
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

      {invitations.length > 0 && (
        <Card className="mb-6">
          <CardTitle>Inviti a collaborare</CardTitle>
          <ul className="space-y-3">
            {invitations.map((inv) => (
              <li key={inv.id} className="flex flex-wrap items-center justify-between gap-3 text-sm">
                <span className="text-foreground">
                  <span className="font-medium">{inv.blog_title}</span> — come{" "}
                  {ROLE_LABELS[inv.role] ?? inv.role} (da @{inv.invited_by_username})
                </span>
                <span className="flex gap-2">
                  <Button onClick={() => respondInvitation(inv.id, "accept")}>Accetta</Button>
                  <Button variant="secondary" onClick={() => respondInvitation(inv.id, "decline")}>
                    Rifiuta
                  </Button>
                </span>
              </li>
            ))}
          </ul>
        </Card>
      )}

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
            <FieldGroup>
              <Label htmlFor="subtitle">Sottotitolo (opzionale, max 64)</Label>
              <Input
                id="subtitle"
                maxLength={64}
                value={subtitle}
                onChange={(e) => setSubtitle(e.target.value)}
              />
            </FieldGroup>
            <FieldGroup>
              <Label htmlFor="default-author-name">Nome pubblico predefinito sugli articoli</Label>
              <Input
                id="default-author-name"
                value={defaultAuthorName}
                onChange={(e) => setDefaultAuthorNameInput(e.target.value)}
              />
            </FieldGroup>
            <FieldGroup>
              <Label htmlFor="visibility">Visibilità</Label>
              <select
                id="visibility"
                value={visibility}
                onChange={(e) => setVisibility(e.target.value as BlogVisibility)}
                className="w-full max-w-xs rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground"
              >
                {(Object.keys(BLOG_VISIBILITY_LABELS) as BlogVisibility[]).map((v) => (
                  <option key={v} value={v}>
                    {BLOG_VISIBILITY_LABELS[v]}
                  </option>
                ))}
              </select>
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
            <Card className="relative h-full overflow-hidden transition hover:border-primary">
              <VisibilityBand visibility={blog.visibility} />
              <h2 className="font-serif text-lg text-foreground">{blog.title}</h2>
              {blog.subtitle && <p className="mt-0.5 text-sm text-foreground/80">{blog.subtitle}</p>}
              <p className="mt-1 text-sm text-muted">{blog.slug}.notturni.eu</p>
              <p className="mt-1 text-xs text-muted">{BLOG_VISIBILITY_LABELS[blog.visibility]}</p>
            </Card>
          </Link>
        ))}
      </div>

      {shared.length > 0 && (
        <>
          <h2 className="mb-4 mt-10 font-serif text-2xl text-foreground">Blog condivisi con me</h2>
          <div className="grid gap-4 sm:grid-cols-2">
            {shared.map(({ blog, role }) => (
              <Link key={blog.id} href={`/dashboard/blogs/${blog.slug}`}>
                <Card className="relative h-full overflow-hidden transition hover:border-primary">
                  <VisibilityBand visibility={blog.visibility} />
                  <h3 className="font-serif text-lg text-foreground">{blog.title}</h3>
                  <p className="mt-1 text-sm text-muted">{blog.slug}.notturni.eu</p>
                  <p className="mt-1 text-xs text-muted">Ruolo: {ROLE_LABELS[role] ?? role}</p>
                </Card>
              </Link>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
