"use client";

import Link from "next/link";
import { useParams } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { Blog, BlogConfig, Comment, Post } from "@/lib/types";

function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

type Tab = "posts" | "settings" | "appearance" | "comments";

const TABS: { id: Tab; label: string }[] = [
  { id: "posts", label: "Post" },
  { id: "comments", label: "Commenti" },
  { id: "appearance", label: "Aspetto" },
  { id: "settings", label: "Impostazioni" },
];

export default function BlogDetailPage() {
  const params = useParams<{ slug: string }>();
  const slug = params.slug;
  const { user } = useAuth();

  const [blog, setBlog] = useState<Blog | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("posts");

  const load = useCallback(() => {
    api.blogs
      .get(slug)
      .then(setBlog)
      .catch((err) => setError(errorMessage(err)));
  }, [slug]);

  useEffect(load, [load]);

  if (error) return <Alert kind="error">{error}</Alert>;
  if (!blog || !user) return <p className="text-sm text-muted">Caricamento…</p>;

  const isOwner = blog.owner_id === user.id;

  return (
    <div>
      <div className="mb-6">
        <Link href="/dashboard" className="text-sm text-muted hover:text-foreground">
          ← I miei blog
        </Link>
        <h1 className="mt-2 font-serif text-2xl text-foreground">{blog.title}</h1>
        <p className="text-sm text-muted">{blog.slug}.notturni.eu</p>
      </div>

      {!isOwner && (
        <Alert kind="info">Non sei il proprietario di questo blog: alcune azioni non sono disponibili.</Alert>
      )}

      <div className="mb-6 mt-4 flex gap-1 border-b border-border">
        {TABS.map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            className={`border-b-2 px-4 py-2 text-sm transition ${
              tab === t.id
                ? "border-primary text-foreground"
                : "border-transparent text-muted hover:text-foreground"
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>

      {tab === "posts" && <PostsTab blogSlug={blog.slug} canWrite={isOwner} />}
      {tab === "comments" && <CommentsTab blogSlug={blog.slug} canModerate={isOwner} />}
      {tab === "appearance" && <AppearanceTab blogSlug={blog.slug} canEdit={isOwner} />}
      {tab === "settings" && <SettingsTab blog={blog} canEdit={isOwner} onUpdated={setBlog} />}
    </div>
  );
}

function PostsTab({ blogSlug, canWrite }: { blogSlug: string; canWrite: boolean }) {
  const { accessToken, authFetch } = useAuth();
  const [posts, setPosts] = useState<Post[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.posts
      .list(accessToken, blogSlug)
      .then(setPosts)
      .catch((err) => setError(errorMessage(err)));
  }, [blogSlug, accessToken]);

  async function handlePublish(postId: string) {
    try {
      const updated = await authFetch((token) => api.posts.publish(token, postId));
      setPosts((prev) => prev?.map((p) => (p.id === postId ? updated : p)) ?? null);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  return (
    <div>
      {canWrite && (
        <div className="mb-4">
          <Link href={`/dashboard/blogs/${blogSlug}/posts/new`}>
            <Button>Nuovo post</Button>
          </Link>
        </div>
      )}
      {error && <Alert kind="error">{error}</Alert>}
      {posts !== null && posts.length === 0 && <p className="text-sm text-muted">Nessun post.</p>}
      <div className="space-y-3">
        {posts?.map((post) => (
          <Card key={post.id} className="flex items-center justify-between">
            <div>
              <Link
                href={`/dashboard/blogs/${blogSlug}/posts/${post.id}`}
                className="font-serif text-lg text-foreground hover:text-primary"
              >
                {post.title}
              </Link>
              <p className="text-sm text-muted">
                {post.locale} · {post.status === "published" ? "pubblicato" : "bozza"}
              </p>
            </div>
            {canWrite && post.status === "draft" && (
              <Button variant="secondary" onClick={() => handlePublish(post.id)}>
                Pubblica
              </Button>
            )}
          </Card>
        ))}
      </div>
    </div>
  );
}

function CommentsTab({ blogSlug, canModerate }: { blogSlug: string; canModerate: boolean }) {
  const { accessToken, authFetch } = useAuth();
  const [pending, setPending] = useState<(Comment & { postTitle: string })[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    if (!canModerate || !accessToken) {
      setPending([]);
      return;
    }
    try {
      const posts = await api.posts.list(accessToken, blogSlug);
      const results = await Promise.all(
        posts.map(async (post) => {
          const comments = await authFetch((token) => api.comments.listPending(token, post.id));
          return comments.map((c) => ({ ...c, postTitle: post.title }));
        })
      );
      setPending(results.flat());
    } catch (err) {
      setError(errorMessage(err));
    }
  }, [blogSlug, canModerate, accessToken, authFetch]);

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- fetch dati all'apertura della tab
    load();
  }, [load]);

  async function handleModerate(commentId: string, action: "approve" | "reject") {
    try {
      await authFetch((token) =>
        action === "approve" ? api.comments.approve(token, commentId) : api.comments.reject(token, commentId)
      );
      setPending((prev) => prev?.filter((c) => c.id !== commentId) ?? null);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  if (!canModerate) return <p className="text-sm text-muted">Solo il proprietario può moderare i commenti.</p>;

  return (
    <div>
      {error && <Alert kind="error">{error}</Alert>}
      {pending !== null && pending.length === 0 && (
        <p className="text-sm text-muted">Nessun commento in attesa di moderazione.</p>
      )}
      <div className="space-y-3">
        {pending?.map((comment) => (
          <Card key={comment.id}>
            <p className="text-xs text-muted">
              su <span className="text-foreground">{comment.postTitle}</span> — da{" "}
              {comment.author_display_name}
            </p>
            <p className="my-2 text-sm text-foreground">{comment.content}</p>
            <div className="flex gap-2">
              <Button onClick={() => handleModerate(comment.id, "approve")}>Approva</Button>
              <Button variant="danger" onClick={() => handleModerate(comment.id, "reject")}>
                Rifiuta
              </Button>
            </div>
          </Card>
        ))}
      </div>
    </div>
  );
}

function AppearanceTab({ blogSlug, canEdit }: { blogSlug: string; canEdit: boolean }) {
  const { authFetch } = useAuth();
  const [config, setConfig] = useState<BlogConfig | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    api.blogs
      .getConfig(blogSlug)
      .then(setConfig)
      .catch((err) => setError(errorMessage(err)));
  }, [blogSlug]);

  function updatePaletteColor(key: string, value: string) {
    setConfig((prev) => ({ ...prev, palette: { ...prev?.palette, [key]: value } }));
    setSaved(false);
  }

  function updateTypography(key: string, value: string) {
    setConfig((prev) => ({ ...prev, typography: { ...prev?.typography, [key]: value } }));
    setSaved(false);
  }

  async function handleSave() {
    if (!config) return;
    setSaving(true);
    setError(null);
    try {
      const updated = await authFetch((token) => api.blogs.updateConfig(token, blogSlug, config));
      setConfig(updated);
      setSaved(true);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSaving(false);
    }
  }

  if (!config) return <p className="text-sm text-muted">Caricamento…</p>;

  const paletteEntries = Object.entries(config.palette ?? {});
  const typographyEntries = Object.entries(config.typography ?? {});

  return (
    <Card>
      <CardTitle>Palette (massimo 5 colori)</CardTitle>
      <div className="mb-6 flex flex-wrap gap-4">
        {paletteEntries.map(([key, value]) => (
          <div key={key}>
            <Label htmlFor={`color-${key}`}>{key}</Label>
            <div className="flex items-center gap-2">
              <input
                id={`color-${key}`}
                type="color"
                value={value}
                onChange={(e) => updatePaletteColor(key, e.target.value)}
                className="h-9 w-9 cursor-pointer rounded border border-border"
              />
              <span className="text-xs text-muted">{value}</span>
            </div>
          </div>
        ))}
      </div>

      <CardTitle>Tipografia (massimo 3 font)</CardTitle>
      <div className="mb-6 flex flex-wrap gap-4">
        {typographyEntries.map(([key, value]) => (
          <FieldGroup key={key}>
            <Label htmlFor={`font-${key}`}>{key}</Label>
            <Input id={`font-${key}`} value={value} onChange={(e) => updateTypography(key, e.target.value)} />
          </FieldGroup>
        ))}
      </div>

      <CardTitle>Layout</CardTitle>
      <FieldGroup>
        <select
          value={config.layout ?? "standard"}
          onChange={(e) => {
            setConfig((prev) => ({ ...prev, layout: e.target.value }));
            setSaved(false);
          }}
          className="w-full max-w-xs rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground"
        >
          <option value="standard">Standard</option>
          <option value="magazine">Magazine</option>
          <option value="minimal">Minimale</option>
        </select>
      </FieldGroup>

      {error && (
        <div className="mb-4">
          <Alert kind="error">{error}</Alert>
        </div>
      )}
      {saved && (
        <div className="mb-4">
          <Alert kind="success">Salvato.</Alert>
        </div>
      )}
      {canEdit && (
        <Button onClick={handleSave} disabled={saving}>
          {saving ? "Salvataggio…" : "Salva aspetto"}
        </Button>
      )}
    </Card>
  );
}

function SettingsTab({
  blog,
  canEdit,
  onUpdated,
}: {
  blog: Blog;
  canEdit: boolean;
  onUpdated: (blog: Blog) => void;
}) {
  const { authFetch } = useAuth();
  const [title, setTitle] = useState(blog.title);
  const [allowAnonymous, setAllowAnonymous] = useState(blog.allow_anonymous_comments);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  async function handleSave() {
    setSaving(true);
    setError(null);
    try {
      const updated = await authFetch((token) =>
        api.blogs.update(token, blog.slug, { title, allow_anonymous_comments: allowAnonymous })
      );
      onUpdated(updated);
      setSaved(true);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Card>
      <FieldGroup>
        <Label htmlFor="blog-title">Titolo</Label>
        <Input id="blog-title" value={title} onChange={(e) => setTitle(e.target.value)} disabled={!canEdit} />
      </FieldGroup>
      <FieldGroup>
        <label className="flex items-center gap-2 text-sm text-foreground">
          <input
            type="checkbox"
            checked={allowAnonymous}
            onChange={(e) => setAllowAnonymous(e.target.checked)}
            disabled={!canEdit}
          />
          Consenti commenti da chi non è registrato (con moderazione obbligatoria)
        </label>
      </FieldGroup>
      <p className="mb-4 text-sm text-muted">Lingua di default: {blog.default_locale}</p>
      {error && (
        <div className="mb-4">
          <Alert kind="error">{error}</Alert>
        </div>
      )}
      {saved && (
        <div className="mb-4">
          <Alert kind="success">Salvato.</Alert>
        </div>
      )}
      {canEdit && (
        <Button onClick={handleSave} disabled={saving}>
          {saving ? "Salvataggio…" : "Salva impostazioni"}
        </Button>
      )}
    </Card>
  );
}
