"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useState } from "react";

import { Button } from "@/components/ui/Button";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";

/** Bottone segui/non seguire del profilo pubblico (mockup 3e). Riceve
 * `initialFollowers` già risolto server-side (`GET /users/{username}/followers`,
 * pubblico) per stabilire lo stato "già seguito" senza una fetch propria —
 * aggiornato poi solo in locale dopo un follow/unfollow. */
export function FollowUserButton({ username, initialFollowers }: { username: string; initialFollowers: string[] }) {
  const { user, loading, authFetch } = useAuth();
  const t = useTranslations("PublicProfile");
  const [followers, setFollowers] = useState<string[]>(initialFollowers);
  const [busy, setBusy] = useState(false);

  const isFollowing = !!user && followers.includes(user.username);
  const canFollow = !!user && user.username !== username;

  async function toggle() {
    if (!user) return;
    setBusy(true);
    try {
      if (isFollowing) {
        await authFetch((token) => api.users.unfollow(token, username));
        setFollowers((prev) => prev.filter((u) => u !== user.username));
      } else {
        await authFetch((token) => api.users.follow(token, username));
        setFollowers((prev) => [...prev, user.username]);
      }
    } finally {
      setBusy(false);
    }
  }

  if (loading) return null;
  if (canFollow) {
    return (
      <Button variant={isFollowing ? "secondary" : "primary"} onClick={toggle} disabled={busy}>
        {isFollowing ? t("unfollow") : t("follow")}
      </Button>
    );
  }
  if (!user) {
    return (
      <Link href="/login">
        <Button variant="secondary">{t("follow")}</Button>
      </Link>
    );
  }
  return null;
}
