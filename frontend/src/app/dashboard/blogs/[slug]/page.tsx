"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useParams, useRouter, useSearchParams } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

import { VisibilityLabel } from "@/components/blog/VisibilityBand";
import { AppearanceTab } from "@/components/dashboard/blog/AppearanceTab";
import { CollaboratorsTab } from "@/components/dashboard/blog/CollaboratorsTab";
import { CommentsTab } from "@/components/dashboard/blog/CommentsTab";
import { MediaTab } from "@/components/dashboard/blog/MediaTab";
import { MyMembershipCard } from "@/components/dashboard/blog/MyMembershipCard";
import { OverviewTab } from "@/components/dashboard/blog/OverviewTab";
import { PagesTab } from "@/components/dashboard/blog/PagesTab";
import { PostsTab } from "@/components/dashboard/blog/PostsTab";
import { SettingsTab } from "@/components/dashboard/blog/SettingsTab";
import { errorMessage } from "@/components/dashboard/blog/shared";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { SkeletonRows } from "@/components/ui/States";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { SITE_HOST } from "@/lib/site";
import { type Blog } from "@/lib/types";

type Tab = "overview" | "posts" | "pages" | "media" | "comments" | "appearance" | "collaborators" | "settings";
const TABS: Tab[] = ["overview", "posts", "pages", "media", "comments", "appearance", "collaborators", "settings"];

/** Scheda del blog in dashboard (mockup 5a-5c/2e/5g): intestazione con
 * visibilità e ruolo, azioni "Vedi il blog"/"Scrivi un post", tab
 * scorrevoli su mobile. La tab attiva è nell'URL (`?tab=`) così le altre
 * schermate possono linkarla. La tab "Panoramica" usa `GET /blogs/{slug}/overview`. */
export default function BlogDetailPage() {
  const params = useParams<{ slug: string }>();
  const slug = params.slug;
  const router = useRouter();
  const searchParams = useSearchParams();
  const { user, accessToken } = useAuth();
  const t = useTranslations("BlogTabs");
  const ta = useTranslations("BlogAdmin");
  const tc = useTranslations("Common");

  const [blog, setBlog] = useState<Blog | null>(null);
  const [error, setError] = useState<string | null>(null);
  const tabParam = searchParams.get("tab");
  const tab: Tab = TABS.includes(tabParam as Tab) ? (tabParam as Tab) : "overview";
  const setTab = (next: Tab) => router.replace(`/dashboard/blogs/${slug}${next === "overview" ? "" : `?tab=${next}`}`);

  const load = useCallback(() => {
    api.blogs
      .get(slug, accessToken)
      .then(setBlog)
      .catch((err) => setError(errorMessage(err)));
  }, [slug, accessToken]);

  useEffect(load, [load]);

  if (error) return <Alert kind="error">{error}</Alert>;
  if (!blog || !user) return <SkeletonRows rows={5} />;

  const isOwner = blog.owner_id === user.id;
  const visibleTabs = TABS.filter((id) => id !== "collaborators" || isOwner);

  return (
    <div className="mx-auto flex max-w-5xl flex-col gap-6">
      <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
        <div className="flex min-w-0 flex-col gap-1.5">
          <Link href="/dashboard" className="text-[13px] text-muted no-underline hover:text-foreground">
            {tc("myDashboard")}
          </Link>
          <h1 className="truncate font-serif text-[28px] font-medium leading-tight text-foreground">{blog.title}</h1>
          <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[13px] text-muted">
            <span className="font-mono">
              {blog.slug}.{SITE_HOST}
            </span>
            <VisibilityLabel visibility={blog.visibility} />
            {blog.deleted_at && <span className="font-semibold text-danger">{ta("deleted")}</span>}
            {blog.is_paused && !blog.deleted_at && <span className="font-semibold text-[#b8862b]">{ta("paused")}</span>}
            {blog.is_suspended && <span className="font-semibold text-danger">{ta("suspended")}</span>}
            {isOwner && <span>{ta("youAre", { role: ta("roles.owner") })}</span>}
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <Link href={`/${blog.slug}`}>
            <Button variant="secondary" size="sm">
              {ta("viewBlog")}
            </Button>
          </Link>
          {isOwner && (
            <Link href={`/dashboard/blogs/${blog.slug}/posts/new`}>
              <Button size="sm">{ta("writePost")}</Button>
            </Link>
          )}
        </div>
      </div>

      {!isOwner && (
        <>
          <Alert kind="info">{ta("notOwner")}</Alert>
          <MyMembershipCard blogSlug={blog.slug} />
        </>
      )}

      <div className="-mx-5 flex gap-1 overflow-x-auto border-b border-border px-5 lg:mx-0 lg:px-0">
        {visibleTabs.map((id) => (
          <button
            key={id}
            type="button"
            onClick={() => setTab(id)}
            className={`whitespace-nowrap border-b-2 px-3.5 py-2 text-sm transition ${
              tab === id ? "border-primary font-medium text-foreground" : "border-transparent text-muted hover:text-foreground"
            }`}
          >
            {t(id)}
          </button>
        ))}
      </div>

      {tab === "overview" && <OverviewTab blogSlug={blog.slug} />}
      {tab === "posts" && <PostsTab blogSlug={blog.slug} canWrite={isOwner} />}
      {tab === "pages" && <PagesTab blog={blog} canWrite={isOwner} />}
      {tab === "media" && <MediaTab blogSlug={blog.slug} canWrite={isOwner} />}
      {tab === "comments" && <CommentsTab blog={blog} canModerate={isOwner} onBlogUpdated={setBlog} />}
      {tab === "appearance" && <AppearanceTab blogSlug={blog.slug} canEdit={isOwner} />}
      {tab === "collaborators" && isOwner && <CollaboratorsTab blogSlug={blog.slug} />}
      {tab === "settings" && <SettingsTab blog={blog} canEdit={isOwner} onUpdated={setBlog} />}
    </div>
  );
}
