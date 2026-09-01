"use client";

import Link from "next/link";
import { useParams } from "next/navigation";
import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import {
  BLOG_VISIBILITY_LABELS,
  INVITABLE_BLOG_ROLES,
  type Blog,
  type BlogConfig,
  type BlogInvitation,
  type BlogMember,
  type BlogRole,
  type BlogVisibility,
  type Category,
  type Comment,
  type Post,
} from "@/lib/types";

function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

const ROLE_LABELS: Record<string, string> = {
  autore: "Autore",
  co_autore: "Co-autore",
  revisore: "Revisore",
  mediatore: "Mediatore",
};

type Tab = "posts" | "comments" | "appearance" | "collaborators" | "settings";

export default function BlogDetailPage() {
  const params = useParams<{ slug: string }>();
  const slug = params.slug;
  const { user, accessToken } = useAuth();

  const [blog, setBlog] = useState<Blog | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("posts");

  const load = useCallback(() => {
    api.blogs
      .get(slug, accessToken)
      .then(setBlog)
      .catch((err) => setError(errorMessage(err)));
  }, [slug, accessToken]);

  useEffect(load, [load]);

  if (error) return <Alert kind="error">{error}</Alert>;
  if (!blog || !user) return <p className="text-sm text-muted">Caricamento…</p>;

  const isOwner = blog.owner_id === user.id;

  const tabs: { id: Tab; label: string }[] = [
    { id: "posts", label: "Post" },
    { id: "comments", label: "Commenti" },
    { id: "appearance", label: "Aspetto" },
    ...(isOwner ? [{ id: "collaborators" as Tab, label: "Collaboratori" }] : []),
    { id: "settings", label: "Impostazioni" },
  ];

  return (
    <div>
      <div className="mb-6">
        <Link href="/dashboard" className="text-sm text-muted hover:text-foreground">
          ← I miei blog
        </Link>
        <h1 className="mt-2 font-serif text-2xl text-foreground">{blog.title}</h1>
        {blog.subtitle && <p className="text-sm text-foreground/80">{blog.subtitle}</p>}
        <p className="text-sm text-muted">
          {blog.slug}.notturni.eu · {BLOG_VISIBILITY_LABELS[blog.visibility]}
        </p>
      </div>

      {!isOwner && (
        <>
          <Alert kind="info">
            Non sei il proprietario di questo blog: alcune azioni non sono disponibili.
          </Alert>
          <div className="mt-4">
            <MyMembershipCard blogSlug={blog.slug} />
          </div>
        </>
      )}

      <div className="mb-6 mt-4 flex gap-1 border-b border-border">
        {tabs.map((t) => (
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
      {tab === "collaborators" && isOwner && <CollaboratorsTab blogSlug={blog.slug} />}
      {tab === "settings" && <SettingsTab blog={blog} canEdit={isOwner} onUpdated={setBlog} />}
    </div>
  );
}

function MyMembershipCard({ blogSlug }: { blogSlug: string }) {
  const { authFetch } = useAuth();
  const [alias, setAlias] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    authFetch((token) => api.blogs.memberOf(token))
      .then((list) => {
        const mine = list.find((m) => m.blog.slug === blogSlug);
        setAlias(mine ? (mine.author_display_name ?? "") : null);
      })
      .catch(() => setAlias(null));
  }, [authFetch, blogSlug]);

  if (alias === null) return null;

  async function handleSave() {
    setError(null);
    try {
      const updated = await authFetch((token) =>
        api.blogs.updateMyMembership(token, blogSlug, alias ?? "")
      );
      setAlias(updated.author_display_name ?? "");
      setSaved(true);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  return (
    <Card>
      <CardTitle>Il mio nome su questo blog</CardTitle>
      <FieldGroup>
        <Label htmlFor="my-alias">Alias autore (todo/BLOG.md #4)</Label>
        <Input
          id="my-alias"
          value={alias}
          maxLength={255}
          placeholder="Lasciare vuoto per usare il nome predefinito del blog o il tuo alias di profilo"
          onChange={(e) => {
            setAlias(e.target.value);
            setSaved(false);
          }}
        />
        <p className="mt-1 text-xs text-muted">
          Con cui firmi i post scritti qui. Ha la precedenza sul nome predefinito del blog e
          sull&apos;alias del tuo profilo.
        </p>
      </FieldGroup>
      {error && (
        <div className="mb-3">
          <Alert kind="error">{error}</Alert>
        </div>
      )}
      {saved && (
        <div className="mb-3">
          <Alert kind="success">Salvato.</Alert>
        </div>
      )}
      <Button onClick={handleSave}>Salva</Button>
    </Card>
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
            <div className="flex items-center gap-3">
              {post.status === "published" && (
                <Link href={post.permalink} className="text-sm text-primary hover:underline">
                  Vedi
                </Link>
              )}
              {canWrite && post.status === "draft" && (
                <Button variant="secondary" onClick={() => handlePublish(post.id)}>
                  Pubblica
                </Button>
              )}
            </div>
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

function CollaboratorsTab({ blogSlug }: { blogSlug: string }) {
  const { authFetch } = useAuth();
  const [members, setMembers] = useState<BlogMember[] | null>(null);
  const [invitations, setInvitations] = useState<BlogInvitation[]>([]);
  const [username, setUsername] = useState("");
  const [role, setRole] = useState<BlogRole>(INVITABLE_BLOG_ROLES[0].value);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    authFetch((token) => api.blogs.members(token, blogSlug))
      .then(setMembers)
      .catch((err) => setError(errorMessage(err)));
    authFetch((token) => api.blogs.listInvitations(token, blogSlug))
      .then(setInvitations)
      .catch(() => undefined);
  }, [authFetch, blogSlug]);

  useEffect(load, [load]);

  async function handleInvite(event: FormEvent) {
    event.preventDefault();
    setError(null);
    try {
      await authFetch((token) => api.blogs.createInvitation(token, blogSlug, username, role));
      setUsername("");
      load();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  async function handleRevoke(invitationId: string) {
    try {
      await authFetch((token) => api.blogs.revokeInvitation(token, blogSlug, invitationId));
      load();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  async function handleRemoveMember(userId: string) {
    try {
      await authFetch((token) => api.blogs.removeMember(token, blogSlug, userId));
      load();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  async function handleChangeRole(userId: string, newRole: BlogRole) {
    try {
      await authFetch((token) => api.blogs.updateMemberRole(token, blogSlug, userId, newRole));
      load();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  const pending = invitations.filter((i) => i.status === "pending");

  return (
    <div className="space-y-6">
      {error && <Alert kind="error">{error}</Alert>}

      <Card>
        <CardTitle>Collaboratori</CardTitle>
        {members !== null && members.length === 0 && (
          <p className="text-sm text-muted">Nessun collaboratore.</p>
        )}
        <ul className="space-y-2">
          {members?.map((m) => (
            <li key={m.user_id} className="flex flex-wrap items-center justify-between gap-3 text-sm">
              <span className="text-foreground">
                @{m.username}
                {m.author_display_name && (
                  <span className="text-muted"> — firma come «{m.author_display_name}»</span>
                )}
              </span>
              <span className="flex items-center gap-2">
                <select
                  value={m.role}
                  onChange={(e) => handleChangeRole(m.user_id, e.target.value as BlogRole)}
                  className="rounded-md border border-border bg-background px-2 py-1 text-sm text-foreground"
                >
                  {INVITABLE_BLOG_ROLES.map((r) => (
                    <option key={r.value} value={r.value}>
                      {r.label}
                    </option>
                  ))}
                </select>
                <button
                  type="button"
                  onClick={() => handleRemoveMember(m.user_id)}
                  className="text-muted hover:text-foreground"
                >
                  Rimuovi
                </button>
              </span>
            </li>
          ))}
        </ul>
      </Card>

      <Card>
        <CardTitle>Invita un collaboratore</CardTitle>
        <p className="mb-4 text-sm text-muted">
          L&apos;invito resta in attesa finché l&apos;utente non lo accetta dalla propria dashboard.
        </p>
        <form onSubmit={handleInvite} className="flex flex-wrap items-end gap-3">
          <div>
            <Label htmlFor="invite-username">Username</Label>
            <Input
              id="invite-username"
              required
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
          </div>
          <div>
            <Label htmlFor="invite-role">Ruolo</Label>
            <select
              id="invite-role"
              value={role}
              onChange={(e) => setRole(e.target.value as BlogRole)}
              className="rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground"
            >
              {INVITABLE_BLOG_ROLES.map((r) => (
                <option key={r.value} value={r.value}>
                  {r.label}
                </option>
              ))}
            </select>
          </div>
          <Button type="submit">Invia invito</Button>
        </form>

        {pending.length > 0 && (
          <ul className="mt-4 space-y-2">
            {pending.map((inv) => (
              <li key={inv.id} className="flex items-center justify-between text-sm">
                <span className="text-foreground">
                  @{inv.invited_username} — {ROLE_LABELS[inv.role] ?? inv.role} (in attesa)
                </span>
                <button
                  type="button"
                  onClick={() => handleRevoke(inv.id)}
                  className="text-muted hover:text-foreground"
                >
                  Revoca
                </button>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}

function AppearanceTab({ blogSlug, canEdit }: { blogSlug: string; canEdit: boolean }) {
  const { accessToken, authFetch } = useAuth();
  const [config, setConfig] = useState<BlogConfig | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    api.blogs
      .getConfig(blogSlug, accessToken)
      .then(setConfig)
      .catch((err) => setError(errorMessage(err)));
  }, [blogSlug, accessToken]);

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
  const [subtitle, setSubtitle] = useState(blog.subtitle ?? "");
  const [description, setDescription] = useState(blog.description ?? "");
  const [visibility, setVisibility] = useState<BlogVisibility>(blog.visibility);
  const [allowAnonymous, setAllowAnonymous] = useState(blog.allow_anonymous_comments);
  const [mentionsEnabled, setMentionsEnabled] = useState(blog.mentions_enabled);
  const [defaultAuthorName, setDefaultAuthorName] = useState(blog.default_author_display_name ?? "");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

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
          allow_anonymous_comments: allowAnonymous,
          mentions_enabled: mentionsEnabled,
          default_author_display_name: defaultAuthorName,
        })
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
    <>
      <Card>
        <FieldGroup>
          <Label htmlFor="blog-title">Titolo</Label>
          <Input id="blog-title" value={title} onChange={(e) => setTitle(e.target.value)} disabled={!canEdit} />
        </FieldGroup>
        <FieldGroup>
          <Label htmlFor="blog-subtitle">Sottotitolo (max 64)</Label>
          <Input
            id="blog-subtitle"
            maxLength={64}
            value={subtitle}
            onChange={(e) => setSubtitle(e.target.value)}
            disabled={!canEdit}
          />
        </FieldGroup>
        <FieldGroup>
          <Label htmlFor="blog-description">Descrizione breve (max 256)</Label>
          <textarea
            id="blog-description"
            maxLength={256}
            rows={3}
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            disabled={!canEdit}
            className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground disabled:opacity-60"
          />
          <p className="mt-1 text-xs text-muted">{description.length}/256</p>
        </FieldGroup>
        <FieldGroup>
          <Label htmlFor="blog-visibility">Visibilità</Label>
          <select
            id="blog-visibility"
            value={visibility}
            onChange={(e) => setVisibility(e.target.value as BlogVisibility)}
            disabled={!canEdit}
            className="w-full max-w-xs rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground disabled:opacity-60"
          >
            {(Object.keys(BLOG_VISIBILITY_LABELS) as BlogVisibility[]).map((v) => (
              <option key={v} value={v}>
                {BLOG_VISIBILITY_LABELS[v]}
              </option>
            ))}
          </select>
          <p className="mt-1 text-xs text-muted">
            <strong>Pubblico</strong>: visibile a tutti e nel feed della homepage.{" "}
            <strong>Solo iscritti</strong>: leggibile solo da chi ha un account.{" "}
            <strong>Privato</strong>: diario visibile e scrivibile solo da te.
          </p>
        </FieldGroup>
        <FieldGroup>
          <Label htmlFor="blog-pen-name">Nome pubblico predefinito per gli autori</Label>
          <Input
            id="blog-pen-name"
            placeholder="es. La redazione — lasciare vuoto per usare lo username"
            value={defaultAuthorName}
            onChange={(e) => setDefaultAuthorName(e.target.value)}
            disabled={!canEdit}
          />
          <p className="mt-1 text-xs text-muted">
            Nome pubblico degli autori sui post di questo blog. Se impostato, vale sempre e non
            è sovrascrivibile dal singolo autore (todo/USERS.md #2).
          </p>
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
        <FieldGroup>
          <label className="flex items-center gap-2 text-sm text-foreground">
            <input
              type="checkbox"
              checked={mentionsEnabled}
              onChange={(e) => setMentionsEnabled(e.target.checked)}
              disabled={!canEdit}
            />
            Trasforma le @menzioni nei post in link al profilo dell&apos;utente citato
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

      <div className="mt-6">
        <CategoriesSettings blogSlug={blog.slug} canEdit={canEdit} />
      </div>
    </>
  );
}

function CategoriesSettings({ blogSlug, canEdit }: { blogSlug: string; canEdit: boolean }) {
  const { authFetch } = useAuth();
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
    <Card>
      <CardTitle>Categorie</CardTitle>
      <p className="mb-4 text-sm text-muted">
        Tassonomia del blog: a differenza dei tag, un post ne ha al più una — pensata per una
        classificazione più organica dei contenuti.
      </p>
      <ul className="mb-4 space-y-2">
        {categories?.map((c) => (
          <li key={c.id} className="flex items-center justify-between text-sm">
            <span className="text-foreground">{c.name}</span>
            {canEdit && (
              <button
                type="button"
                onClick={() => handleDelete(c.id)}
                className="text-muted hover:text-foreground"
              >
                Rimuovi
              </button>
            )}
          </li>
        ))}
        {categories?.length === 0 && <p className="text-sm text-muted">Nessuna categoria.</p>}
      </ul>
      {canEdit && (
        <form onSubmit={handleAdd} className="flex flex-wrap items-end gap-3">
          <div>
            <Label htmlFor="cat-name">Nome</Label>
            <Input id="cat-name" required value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <div>
            <Label htmlFor="cat-slug">Slug</Label>
            <Input id="cat-slug" required value={slug} onChange={(e) => setSlug(e.target.value)} />
          </div>
          <Button type="submit">Aggiungi</Button>
        </form>
      )}
      {error && (
        <div className="mt-3">
          <Alert kind="error">{error}</Alert>
        </div>
      )}
    </Card>
  );
}
