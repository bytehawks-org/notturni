"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Pill } from "@/components/ui/Pill";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { type Post } from "@/lib/types";

import { errorMessage } from "./shared";

const STATUS_LABEL: Record<Post["status"], string> = {
  draft: "Bozza",
  published: "Pubblicato",
};
const STATUS_TONE: Record<Post["status"], "ok" | "neutral"> = {
  draft: "neutral",
  published: "ok",
};

export function PostsTab({ blogSlug, canWrite }: { blogSlug: string; canWrite: boolean }) {
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
      <div className="flex flex-col gap-2.5">
        {posts?.map((post) => (
          <Card key={post.id} className="flex items-center justify-between p-4">
            <div className="flex flex-col gap-1">
              <Link
                href={`/dashboard/blogs/${blogSlug}/posts/${post.id}`}
                className="font-serif text-lg text-foreground hover:text-primary"
              >
                {post.title}
              </Link>
              <p className="flex items-center gap-2 text-sm text-muted">
                <span>{post.locale}</span>
                <Pill tone={STATUS_TONE[post.status]}>{STATUS_LABEL[post.status]}</Pill>
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
