"use client";

import { useLocale, useTranslations } from "next-intl";
import Link from "next/link";
import { useCallback, useEffect, useMemo, useState, type FormEvent } from "react";

import { BlogCard } from "@/components/blog/VisibilityBand";
import { StatusPill } from "@/components/blog/StatusPill";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { FilterChip } from "@/components/ui/Pill";
import { EmptyState, SkeletonCards, SkeletonRows } from "@/components/ui/States";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import { displayPostStatus, type DisplayPostStatus } from "@/lib/post-status";
import type { Blog, BlogComment, BlogInvitation, BlogVisibility, MembershipBlog, Post } from "@/lib/types";

const MAX_BLOGS = 5;
const VISIBILITIES: BlogVisibility[] = ["public", "members", "private"];
const POST_FILTERS: ("all" | DisplayPostStatus)[] = ["all", "draft", "review", "scheduled", "published"];

/** Dashboard utente (mockup 1e desktop / 1f mobile): saluto e riepilogo,
 * KPI, post recenti di tutti i propri blog, inviti, commenti da moderare,
 * card dei blog con banda di visibilità. I contatori aggregati (letture,
 * nuovi follower della settimana) arrivano con il blocco B1. */
export default function DashboardHomePage() {
  const { user, authFetch } = useAuth();
  const t = useTranslations("Dashboard");
  const tc = useTranslations("Common");
  const tv = useTranslations("Visibility");
  const tr = useTranslations("BlogAdmin");
  const locale = useLocale();

  const [blogs, setBlogs] = useState<Blog[] | null>(null);
  const [shared, setShared] = useState<MembershipBlog[]>([]);
  const [invitations, setInvitations] = useState<BlogInvitation[]>([]);
  const [posts, setPosts] = useState<Post[] | null>(null);
  const [pendingComments, setPendingComments] = useState<(BlogComment & { blog_slug: string; blog_title: string })[]>([]);
  const [followers, setFollowers] = useState<number | null>(null);
  const [postFilter, setPostFilter] = useState<(typeof POST_FILTERS)[number]>("all");
  const [error, setError] = useState<string | null>(null);

  const [showCreate, setShowCreate] = useState(false);
  const [slug, setSlug] = useState("");
  const [title, setTitle] = useState("");
  const [subtitle, setSubtitle] = useState("");
  const [visibility, setVisibility] = useState<BlogVisibility>("public");
  // Alias di default del blog: se lasciato vuoto il backend ricade sullo
  // username (vedi _resolve_author_display_name), il suggerimento lo esplicita.
  const [defaultAuthorNameInput, setDefaultAuthorNameInput] = useState<string | null>(null);
  const defaultAuthorName = defaultAuthorNameInput ?? user?.username ?? "";
  const [createError, setCreateError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const errorMessage = useCallback(
    (err: unknown) => (err instanceof ApiClientError ? err.message : tc("unexpectedError")),
    [tc]
  );

  const load = useCallback(() => {
    authFetch((token) => api.blogs.listMine(token))
      .then(async (mine) => {
        setBlogs(mine);
        const [postLists, commentLists] = await Promise.all([
          Promise.all(mine.map((b) => authFetch((token) => api.posts.list(token, b.slug)).catch(() => [] as Post[]))),
          Promise.all(
            mine.map((b) =>
              authFetch((token) => api.comments.listForBlog(token, b.slug, "pending"))
                .then((list) => list.map((c) => ({ ...c, blog_slug: b.slug, blog_title: b.title })))
                .catch(() => [])
            )
          ),
        ]);
        setPosts(postLists.flat().sort((a, b) => b.created_at.localeCompare(a.created_at)));
        setPendingComments(commentLists.flat());
      })
      .catch((err) => setError(errorMessage(err)));
    authFetch((token) => api.blogs.memberOf(token))
      .then(setShared)
      .catch(() => undefined);
    authFetch((token) => api.blogs.receivedInvitations(token))
      .then(setInvitations)
      .catch(() => undefined);
    authFetch((token) => api.users.followStats(token))
      .then((stats) => setFollowers(stats.total_followers))
      .catch(() => undefined);
  }, [authFetch, errorMessage]);

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
      setCreateError(errorMessage(err));
    } finally {
      setSubmitting(false);
    }
  }

  async function respondInvitation(id: string, action: "accept" | "decline") {
    try {
      await authFetch((token) =>
        action === "accept" ? api.blogs.acceptInvitation(token, id) : api.blogs.declineInvitation(token, id)
      );
      load();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  const drafts = useMemo(() => (posts ?? []).filter((p) => displayPostStatus(p) === "draft").length, [posts]);
  const visiblePosts = useMemo(
    () => (posts ?? []).filter((p) => postFilter === "all" || displayPostStatus(p) === postFilter).slice(0, 8),
    [posts, postFilter]
  );
  const blogTitle = (blogId: string) => blogs?.find((b) => b.id === blogId)?.title ?? "";
  const blogSlugOf = (blogId: string) => blogs?.find((b) => b.id === blogId)?.slug ?? "";
  const hour = new Date().getHours();
  const greeting = hour < 12 ? t("goodMorning") : hour < 18 ? t("goodAfternoon") : t("goodEvening");
  const left = MAX_BLOGS - (blogs?.length ?? 0);
  const firstBlog = blogs?.[0];

  return (
    <div className="mx-auto flex max-w-5xl flex-col gap-8">
      <header className="flex flex-col gap-4 md:flex-row md:items-end md:justify-between">
        <div className="flex flex-col gap-1.5">
          <h1 className="font-serif text-[28px] font-medium leading-tight text-foreground md:text-[32px]">
            {greeting}, {user?.username}
          </h1>
          <p className="text-sm text-muted">
            {posts === null
              ? tc("loading")
              : [
                  t("draftsWaiting", { count: drafts }),
                  invitations.length > 0 ? t("invitesPending", { count: invitations.length }) : null,
                  pendingComments.length > 0 ? t("commentsPending", { count: pendingComments.length }) : null,
                ]
                  .filter(Boolean)
                  .join(" · ")}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          {!showCreate && left > 0 && (
            <Button variant="secondary" onClick={() => setShowCreate(true)}>
              {t("newBlog")} <span className="ml-1 text-muted">· {t("blogsLeft", { count: left })}</span>
            </Button>
          )}
          {firstBlog && (
            <Link href={`/dashboard/blogs/${firstBlog.slug}/posts/new`}>
              <Button>{tr("writePost")}</Button>
            </Link>
          )}
        </div>
      </header>

      {error && <Alert kind="error">{error}</Alert>}

      {blogs !== null && blogs.length > 0 && (
        <div className="grid grid-cols-3 gap-3">
          {[
            [posts?.length ?? "…", t("kpiPosts")],
            [followers ?? "…", t("kpiFollowers")],
            [blogs.length + shared.length, t("kpiBlogs")],
          ].map(([value, label]) => (
            <div key={String(label)} className="flex flex-col gap-1 rounded-xl border border-border bg-surface px-4 py-3.5">
              <span className="font-serif text-2xl text-foreground">{value}</span>
              <span className="text-xs text-muted">{label}</span>
            </div>
          ))}
        </div>
      )}

      {invitations.length > 0 && (
        <Card className="flex flex-col gap-3 border-primary/40">
          <span className="font-mono text-[11px] uppercase tracking-[.08em] text-primary">{t("invitation")}</span>
          <ul className="flex flex-col gap-3">
            {invitations.map((inv) => (
              <li key={inv.id} className="flex flex-wrap items-center justify-between gap-3 text-sm">
                <span className="text-foreground">
                  {t.rich("invitationLine", {
                    by: inv.invited_by_username,
                    blog: inv.blog_title,
                    role: tr(`roles.${inv.role === "co_autore" ? "co-author" : inv.role === "revisore" ? "reviewer" : inv.role === "mediatore" ? "mediator" : "author"}`),
                    b: (chunks) => <span className="font-medium">{chunks}</span>,
                  })}
                </span>
                <span className="flex gap-2">
                  <Button size="sm" onClick={() => respondInvitation(inv.id, "accept")}>
                    {t("accept")}
                  </Button>
                  <Button size="sm" variant="secondary" onClick={() => respondInvitation(inv.id, "decline")}>
                    {t("decline")}
                  </Button>
                </span>
              </li>
            ))}
          </ul>
        </Card>
      )}

      {showCreate && (
        <Card>
          <CardTitle>{t("newBlog")}</CardTitle>
          <form onSubmit={handleCreate}>
            <FieldGroup>
              <Label htmlFor="slug" hint={t("slugHint")}>
                {t("slug")}
              </Label>
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
              <Label htmlFor="title">{t("title")}</Label>
              <Input id="title" required value={title} onChange={(e) => setTitle(e.target.value)} />
            </FieldGroup>
            <FieldGroup>
              <Label htmlFor="subtitle">{t("subtitle")}</Label>
              <Input id="subtitle" maxLength={64} value={subtitle} onChange={(e) => setSubtitle(e.target.value)} />
            </FieldGroup>
            <FieldGroup>
              <Label htmlFor="default-author-name">{t("defaultAuthor")}</Label>
              <Input id="default-author-name" value={defaultAuthorName} onChange={(e) => setDefaultAuthorNameInput(e.target.value)} />
            </FieldGroup>
            <FieldGroup>
              <Label htmlFor="visibility">{t("visibility")}</Label>
              <select
                id="visibility"
                value={visibility}
                onChange={(e) => setVisibility(e.target.value as BlogVisibility)}
                className="w-full max-w-xs rounded-lg border border-border bg-surface px-3 py-2.5 text-sm text-foreground focus:outline-none focus:border-primary focus:ring-[3px] focus:ring-primary/20"
              >
                {VISIBILITIES.map((v) => (
                  <option key={v} value={v}>
                    {tv(v)}
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
                {submitting ? t("creating") : t("create")}
              </Button>
              <Button type="button" variant="secondary" onClick={() => setShowCreate(false)}>
                {tc("cancel")}
              </Button>
            </div>
          </form>
        </Card>
      )}

      <section className="flex flex-col gap-3">
        <div className="flex items-baseline justify-between">
          <h2 className="font-serif text-xl text-foreground">{t("myBlogs")}</h2>
        </div>
        {blogs === null && !error && <SkeletonCards count={2} />}
        {blogs !== null && blogs.length === 0 && (
          <EmptyState
            glyph="✎"
            title={t("noBlogsTitle")}
            body={t("noBlogsBody")}
            action={!showCreate ? <Button onClick={() => setShowCreate(true)}>{t("newBlog")}</Button> : undefined}
          />
        )}
        <div className="grid gap-4 sm:grid-cols-2">
          {blogs?.map((blog) => (
            <Link key={blog.id} href={`/dashboard/blogs/${blog.slug}`} className="no-underline">
              <BlogCard
                name={blog.title}
                slug={blog.slug}
                subtitle={blog.subtitle}
                visibility={blog.visibility}
                meta={
                  <>
                    <span>{t("postsInBlog", { count: (posts ?? []).filter((p) => p.blog_id === blog.id).length })}</span>
                    {blog.deleted_at && <span className="font-semibold text-danger">{t("deletedBadge")}</span>}
                    {blog.is_paused && !blog.deleted_at && <span className="font-semibold text-[#b8862b]">{t("pausedBadge")}</span>}
                  </>
                }
              />
            </Link>
          ))}
        </div>
      </section>

      {blogs !== null && blogs.length > 0 && (
        <section className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <h2 className="font-serif text-xl text-foreground">{t("recentPosts")}</h2>
            <div className="flex gap-1.5 overflow-x-auto">
              {POST_FILTERS.map((f) => (
                <FilterChip key={f} active={postFilter === f} onClick={() => setPostFilter(f)}>
                  {t(`filter.${f}`)}
                </FilterChip>
              ))}
            </div>
          </div>
          {posts === null ? (
            <SkeletonRows rows={4} />
          ) : visiblePosts.length === 0 ? (
            <p className="text-sm text-muted">{t("noPosts")}</p>
          ) : (
            <ul className="flex flex-col rounded-xl border border-border bg-surface">
              {visiblePosts.map((post) => (
                <li key={post.id} className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-b border-border px-4 py-3 last:border-0">
                  <div className="flex min-w-0 flex-col gap-0.5">
                    <Link
                      href={`/dashboard/blogs/${blogSlugOf(post.blog_id)}/posts/${post.id}`}
                      className="truncate font-serif text-[17px] text-foreground no-underline hover:text-primary"
                    >
                      {post.title}
                    </Link>
                    <span className="truncate text-[13px] text-muted">
                      {blogTitle(post.blog_id)} · {post.locale.toUpperCase()} ·{" "}
                      {formatDate(post.published_at ?? post.created_at, locale, { day: "numeric", month: "short", year: "numeric" })}
                    </span>
                  </div>
                  <StatusPill status={displayPostStatus(post)} />
                </li>
              ))}
            </ul>
          )}
        </section>
      )}

      {pendingComments.length > 0 && (
        <section className="flex flex-col gap-3">
          <h2 className="font-serif text-xl text-foreground">
            {t("commentsToModerate")} <span className="text-muted">· {pendingComments.length}</span>
          </h2>
          <ul className="flex flex-col gap-2">
            {pendingComments.slice(0, 3).map((c) => (
              <li key={c.id} className="flex flex-wrap items-baseline justify-between gap-2 rounded-xl border border-border bg-surface px-4 py-3 text-sm">
                <span className="min-w-0 flex-1 truncate">
                  “{c.content}” <span className="text-muted">— {c.author_display_name}</span>
                </span>
                <Link href={`/dashboard/blogs/${c.blog_slug}?tab=comments`} className="text-[13px] text-primary no-underline hover:underline">
                  {c.blog_title}
                </Link>
              </li>
            ))}
          </ul>
        </section>
      )}

      {shared.length > 0 && (
        <section className="flex flex-col gap-3">
          <h2 className="font-serif text-xl text-foreground">{t("sharedWithMe")}</h2>
          <div className="grid gap-4 sm:grid-cols-2">
            {shared.map(({ blog, role }) => (
              <Link key={blog.id} href={`/dashboard/blogs/${blog.slug}`} className="no-underline">
                <BlogCard
                  name={blog.title}
                  slug={blog.slug}
                  visibility={blog.visibility}
                  meta={<span>{tr("youAre", { role: tr(`roles.${role === "co_autore" ? "co-author" : role === "revisore" ? "reviewer" : role === "mediatore" ? "mediator" : "author"}`) })}</span>}
                />
              </Link>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
