"use client";

import Image from "next/image";
import { useParams } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { languageName } from "@/lib/languages";
import { getSocialPlatform } from "@/lib/social-platforms";
import type { Profile } from "@/lib/types";

const countryNames = new Intl.DisplayNames(["it"], { type: "region" });

function countryName(code: string): string {
  try {
    return countryNames.of(code) ?? code;
  } catch {
    return code;
  }
}

function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

export default function PublicProfilePage() {
  const params = useParams<{ username: string }>();
  const { user, authFetch } = useAuth();

  const [profile, setProfile] = useState<Profile | null>(null);
  const [followers, setFollowers] = useState<string[]>([]);
  const [isFollowing, setIsFollowing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    api.users
      .profile(params.username)
      .then(setProfile)
      .catch((err) => setError(errorMessage(err)));
    api.users
      .followers(params.username)
      .then((list) => {
        const usernames = list.map((f) => f.username);
        setFollowers(usernames);
        if (user) setIsFollowing(usernames.includes(user.username));
      })
      .catch(() => undefined);
  }, [params.username, user]);

  useEffect(load, [load]);

  async function handleFollowToggle() {
    try {
      if (isFollowing) {
        await authFetch((token) => api.users.unfollow(token, params.username));
      } else {
        await authFetch((token) => api.users.follow(token, params.username));
      }
      load();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  if (error) return <Alert kind="error">{error}</Alert>;
  if (!profile) return <p className="text-sm text-muted">Caricamento…</p>;

  const canFollow = user && user.username !== params.username;

  return (
    <main className="mx-auto max-w-2xl px-6 py-16">
      <Card>
        <div className="flex items-center gap-4">
          {profile.avatar_url ? (
            <Image
              src={profile.avatar_url}
              alt={profile.username}
              width={72}
              height={72}
              className="h-18 w-18 rounded-full object-cover"
              unoptimized
            />
          ) : (
            <div className="flex h-18 w-18 items-center justify-center rounded-full bg-foreground/10 text-xl text-muted">
              {profile.username[0]?.toUpperCase()}
            </div>
          )}
          <div>
            <h1 className="font-serif text-2xl text-foreground">
              {profile.first_name || profile.last_name
                ? [profile.first_name, profile.last_name].filter(Boolean).join(" ")
                : profile.username}
            </h1>
            <p className="text-sm text-muted">
              {profile.first_name || profile.last_name ? `@${profile.username} · ` : ""}
              {followers.length} follower
            </p>
          </div>
          {canFollow && (
            <div className="ml-auto">
              <Button variant={isFollowing ? "secondary" : "primary"} onClick={handleFollowToggle}>
                {isFollowing ? "Non seguire più" : "Segui"}
              </Button>
            </div>
          )}
        </div>

        {(profile.country || profile.native_language || profile.fallback_languages.length > 0) && (
          <p className="mt-4 text-sm text-muted">
            {profile.country && countryName(profile.country)}
            {profile.native_language && (
              <>
                {profile.country && " · "}
                Lingua madre: {languageName(profile.native_language)}
              </>
            )}
            {profile.fallback_languages.length > 0 && (
              <>
                {" · "}
                Traduce anche in: {profile.fallback_languages.map(languageName).join(", ")}
              </>
            )}
          </p>
        )}

        {profile.bio && <p className="mt-4 text-sm text-foreground">{profile.bio}</p>}

        {profile.social_links.length > 0 && (
          <ul className="mt-4 flex flex-wrap gap-4">
            {profile.social_links.map((link) => {
              const platform = getSocialPlatform(link.label);
              return (
                <li key={link.id}>
                  <a
                    href={link.url}
                    target="_blank"
                    rel="noopener noreferrer"
                    title={platform.label}
                    className="flex items-center gap-1.5 text-sm text-muted hover:text-primary"
                  >
                    <platform.Icon />
                    {platform.label}
                  </a>
                </li>
              );
            })}
          </ul>
        )}
      </Card>
    </main>
  );
}
