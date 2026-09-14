"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/Button";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";

/** "Segui · N" nell'header del blog (mockup 1a/3f). Il conteggio viene
 * dall'elenco pubblico dei follower (`GET /blogs/{slug}/followers`); lo stato
 * "già seguito" si deduce dalla presenza del proprio username in quell'elenco. */
export function FollowBlogButton({ slug }: { slug: string }) {
  const { user, loading, authFetch } = useAuth();
  const t = useTranslations("BlogPage");
  const [followers, setFollowers] = useState<string[] | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.blogs
      .followers(slug)
      .then((list) => setFollowers(list.map((f) => f.username)))
      .catch(() => setFollowers([]));
  }, [slug]);

  const isFollowing = !!user && !!followers?.includes(user.username);
  const count = followers?.length ?? null;
  const label = `${isFollowing ? t("following") : t("follow")}${count !== null ? ` · ${count}` : ""}`;

  async function toggle() {
    if (!user) return;
    setBusy(true);
    try {
      if (isFollowing) {
        await authFetch((token) => api.blogs.unfollow(token, slug));
        setFollowers((prev) => prev?.filter((u) => u !== user.username) ?? null);
      } else {
        await authFetch((token) => api.blogs.follow(token, slug));
        setFollowers((prev) => [...(prev ?? []), user.username]);
      }
    } catch {
      // errore già segnalato dal backend con 4xx; qui nessun feedback aggiuntivo
    } finally {
      setBusy(false);
    }
  }

  if (loading) return null;
  if (!user) {
    return (
      <Link href="/login">
        <Button size="sm" variant="secondary">
          {label}
        </Button>
      </Link>
    );
  }
  return (
    <Button size="sm" variant={isFollowing ? "secondary" : "primary"} onClick={toggle} disabled={busy}>
      {label}
    </Button>
  );
}
