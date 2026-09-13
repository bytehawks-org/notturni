"use client";

import { useLocale, useTranslations } from "next-intl";
import Image from "next/image";
import Link from "next/link";
import { useParams } from "next/navigation";
import { useCallback, useEffect, useState } from "react";

import { SiteHeader } from "@/components/SiteHeader";
import { SiteFooter } from "@/components/shell/SiteFooter";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { SkeletonRows } from "@/components/ui/States";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import { languageName } from "@/lib/languages";
import { getSocialPlatform } from "@/lib/social-platforms";
import type { Profile } from "@/lib/types";

function countryName(code: string, locale: string): string {
  try {
    return new Intl.DisplayNames([locale], { type: "region" }).of(code) ?? code;
  } catch {
    return code;
  }
}

/** Profilo pubblico (mockup 3e): intestazione con avatar, luogo e lingue,
 * bio, statistiche e link social. Le tab Post/Blog/Commenti richiedono
 * endpoint per utente non ancora esposti (blocco B1). */
export default function PublicProfilePage() {
  const params = useParams<{ username: string }>();
  const { user, authFetch } = useAuth();
  const t = useTranslations("PublicProfile");
  const tc = useTranslations("Common");
  const locale = useLocale();

  const [profile, setProfile] = useState<Profile | null>(null);
  const [followers, setFollowers] = useState<string[]>([]);
  const [isFollowing, setIsFollowing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    api.users
      .profile(params.username)
      .then(setProfile)
      .catch((err) => setError(err instanceof ApiClientError ? err.message : tc("unexpectedError")));
    api.users
      .followers(params.username)
      .then((list) => {
        const usernames = list.map((f) => f.username);
        setFollowers(usernames);
        if (user) setIsFollowing(usernames.includes(user.username));
      })
      .catch(() => undefined);
  }, [params.username, user, tc]);

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
      setError(err instanceof ApiClientError ? err.message : tc("unexpectedError"));
    }
  }

  const canFollow = user && user.username !== params.username;
  // todo/BLOG.md #4: l'alias pubblico ha la precedenza su nome/cognome e username.
  const displayHeading =
    profile?.display_name ||
    (profile?.first_name || profile?.last_name
      ? [profile?.first_name, profile?.last_name].filter(Boolean).join(" ")
      : profile?.username);
  const languages = profile ? [profile.native_language, ...profile.fallback_languages].filter((l): l is string => !!l) : [];

  return (
    <>
      <SiteHeader />
      <main className="mx-auto w-full max-w-[860px] flex-1 px-5 py-10 lg:px-12 lg:py-14">
        {error && <Alert kind="error">{error}</Alert>}
        {!profile && !error && <SkeletonRows rows={3} />}
        {profile && (
          <div className="flex flex-col gap-8">
            <header className="grid gap-5 md:grid-cols-[96px_minmax(0,1fr)_auto] md:items-start md:gap-7">
              {profile.avatar_url ? (
                <Image src={profile.avatar_url} alt={profile.username} width={96} height={96} className="h-24 w-24 rounded-full object-cover" unoptimized />
              ) : (
                <div className="flex h-24 w-24 items-center justify-center rounded-full bg-primary font-serif text-3xl text-background">
                  {profile.username[0]?.toUpperCase()}
                </div>
              )}
              <div className="flex min-w-0 flex-col gap-2">
                <h1 className="font-serif text-[30px] font-medium leading-tight text-foreground">{displayHeading}</h1>
                <p className="flex flex-wrap items-center gap-x-2 text-sm text-muted">
                  <span>@{profile.username}</span>
                  {profile.country && <span>· {countryName(profile.country, locale)}</span>}
                  {languages.length > 0 && (
                    <span>
                      · {t("writesIn")} {languages.map((code) => languageName(code, locale)).join(", ")}
                    </span>
                  )}
                </p>
                {profile.bio && <p className="max-w-[560px] text-[15px] leading-relaxed text-foreground">{profile.bio}</p>}
                <div className="flex flex-wrap items-center gap-5 pt-1 text-sm">
                  <span>
                    <span className="font-semibold text-foreground">{followers.length}</span>{" "}
                    <span className="text-muted">{t("followers", { count: followers.length })}</span>
                  </span>
                  <span className="text-muted">
                    {t("memberSince", { date: formatDate(profile.created_at, locale, { month: "long", year: "numeric" }) })}
                  </span>
                </div>
              </div>
              {canFollow ? (
                <Button variant={isFollowing ? "secondary" : "primary"} onClick={handleFollowToggle}>
                  {isFollowing ? t("unfollow") : t("follow")}
                </Button>
              ) : !user ? (
                <Link href="/login">
                  <Button variant="secondary">{t("follow")}</Button>
                </Link>
              ) : null}
            </header>

            {profile.social_links.length > 0 && (
              <section className="flex flex-col gap-2">
                <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("links")}</span>
                <ul className="flex flex-wrap gap-4">
                  {profile.social_links.map((link) => {
                    const platform = getSocialPlatform(link.label);
                    return (
                      <li key={link.id}>
                        <a
                          href={link.url}
                          target="_blank"
                          rel="noopener noreferrer"
                          title={platform.label}
                          className="flex items-center gap-1.5 text-sm text-muted no-underline hover:text-primary"
                        >
                          <platform.Icon />
                          {platform.label}
                        </a>
                      </li>
                    );
                  })}
                </ul>
              </section>
            )}

            <p className="text-[13px] leading-relaxed text-muted">{t("privacyNote")}</p>
          </div>
        )}
      </main>
      <SiteFooter />
    </>
  );
}
