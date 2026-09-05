"use client";

import Link from "next/link";
import { useParams } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

import { AppearanceTab } from "@/components/dashboard/blog/AppearanceTab";
import { CollaboratorsTab } from "@/components/dashboard/blog/CollaboratorsTab";
import { CommentsTab } from "@/components/dashboard/blog/CommentsTab";
import { MyMembershipCard } from "@/components/dashboard/blog/MyMembershipCard";
import { PagesTab } from "@/components/dashboard/blog/PagesTab";
import { PostsTab } from "@/components/dashboard/blog/PostsTab";
import { SettingsTab } from "@/components/dashboard/blog/SettingsTab";
import { errorMessage } from "@/components/dashboard/blog/shared";
import { Alert } from "@/components/ui/Alert";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { BLOG_VISIBILITY_LABELS, type Blog } from "@/lib/types";

type Tab = "posts" | "pages" | "comments" | "appearance" | "collaborators" | "settings";

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
    { id: "pages", label: "Pagine" },
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
      {tab === "pages" && <PagesTab blog={blog} canWrite={isOwner} />}
      {tab === "comments" && <CommentsTab blogSlug={blog.slug} canModerate={isOwner} />}
      {tab === "appearance" && <AppearanceTab blogSlug={blog.slug} canEdit={isOwner} />}
      {tab === "collaborators" && isOwner && <CollaboratorsTab blogSlug={blog.slug} />}
      {tab === "settings" && <SettingsTab blog={blog} canEdit={isOwner} onUpdated={setBlog} />}
    </div>
  );
}
