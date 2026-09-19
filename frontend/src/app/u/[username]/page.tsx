import { getLocale, getTranslations } from "next-intl/server";
import Image from "next/image";
import Link from "next/link";
import { notFound } from "next/navigation";

import { FeedPostCard } from "@/components/FeedPostCard";
import { BlogDirectoryGridClient } from "@/components/home/BlogDirectoryClient";
import { FollowUserButton } from "@/components/profile/FollowUserButton";
import { ProfileTabs } from "@/components/profile/ProfileTabs";
import { SiteHeader } from "@/components/SiteHeader";
import { EmptyState } from "@/components/ui/States";
import { VerificationBadge } from "@/components/ui/VerificationBadge";
import { formatDate } from "@/lib/format";
import { interestLabel, interestsByKey } from "@/lib/interests";
import { languageName } from "@/lib/languages";
import {
  getPublicInterests,
  getPublicUserBlogs,
  getPublicUserComments,
  getPublicUserFollowers,
  getPublicUserPosts,
  getPublicUserProfile,
} from "@/lib/server-api";
import { getSocialPlatform } from "@/lib/social-platforms";
import type { Profile } from "@/lib/types";

interface PageParams {
  username: string;
}

function countryName(code: string, locale: string): string {
  try {
    return new Intl.DisplayNames([locale], { type: "region" }).of(code) ?? code;
  } catch {
    return code;
  }
}

/** Nome "personale" mostrato in testa al profilo pubblico: stessa
 * preferenza `post_author_name_style` usata per firmare i post
 * (dashboard/profilo, "Firma i miei post come" — mirror di
 * `app/domain/display_names.py::resolve_personal_display_name`, che qui non
 * si può importare da un Server Component frontend). Lo username resta
 * comunque sempre visibile sotto, come @username. */
function resolvePersonalDisplayName(profile: Profile): string {
  if (profile.post_author_name_style === "full_name") {
    const full = [profile.first_name, profile.last_name].filter(Boolean).join(" ").trim();
    return full || profile.username;
  }
  if (profile.post_author_name_style === "display_name") {
    return profile.display_name || profile.username;
  }
  if (profile.post_author_name_style === "verified_domain") {
    return profile.custom_domain || profile.username;
  }
  return profile.username;
}

/** Profilo pubblico (mockup 3e): intestazione con avatar, luogo e lingue,
 * bio, statistiche, link social e tab Post/Blog/Commenti
 * (`GET /users/{username}/posts|blogs|comments`, solo contenuti firmati con
 * lo username — CLAUDE.md #8). Server Component: tutto il contenuto è
 * risolto lato server (SEO — prima era interamente client-side, invisibile
 * ai crawler che non eseguono JS). Solo il follow/unfollow resta un client
 * component isolato (`FollowUserButton`), richiede la sessione del
 * visitatore, mai disponibile a un Server Component. */
export default async function PublicProfilePage({ params }: { params: Promise<PageParams> }) {
  const { username } = await params;
  const [profile, posts, blogs, comments, followers, interestsList, locale, t, tTier] = await Promise.all([
    getPublicUserProfile(username),
    getPublicUserPosts(username),
    getPublicUserBlogs(username),
    getPublicUserComments(username),
    getPublicUserFollowers(username),
    getPublicInterests(),
    getLocale(),
    getTranslations("PublicProfile"),
    getTranslations("VerificationTier"),
  ]);
  if (!profile) notFound();

  const displayHeading = resolvePersonalDisplayName(profile);
  const languages = [profile.native_language, ...profile.fallback_languages].filter((l): l is string => !!l);
  const interestsMap = interestsByKey(interestsList);

  return (
    <>
      <SiteHeader />
      <main className="mx-auto w-full max-w-[860px] flex-1 px-5 py-10 lg:px-12 lg:py-14">
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
              <h1 className="flex items-center gap-2 font-serif text-[30px] font-medium leading-tight text-foreground">
                {displayHeading}
                <VerificationBadge
                  tier={profile.verification_tier}
                  size={20}
                  label={profile.verification_tier !== "none" ? tTier(profile.verification_tier) : undefined}
                />
              </h1>
              <p className="flex flex-wrap items-center gap-x-2 text-sm text-muted">
                <span>@{profile.custom_domain ?? profile.username}</span>
                {profile.country && <span>· {countryName(profile.country, locale)}</span>}
                {languages.length > 0 && (
                  <span>
                    · {t("writesIn")} {languages.map((code) => languageName(code, locale)).join(", ")}
                  </span>
                )}
              </p>
              {profile.bio && <p className="max-w-[560px] text-[15px] leading-relaxed text-foreground">{profile.bio}</p>}
              {profile.interests.length > 0 && (
                <div className="flex flex-wrap gap-1.5">
                  {profile.interests.map((key) => (
                    <Link
                      key={key}
                      href={`/users?interest=${encodeURIComponent(key)}`}
                      className="rounded-full border border-border px-2.5 py-0.5 text-[13px] text-muted no-underline hover:border-primary/40 hover:text-foreground"
                    >
                      {interestLabel(interestsMap.get(key), key, locale)}
                    </Link>
                  ))}
                </div>
              )}
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
            <FollowUserButton username={username} initialFollowers={followers} />
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

          <ProfileTabs
            tabs={[
              {
                id: "posts",
                label: t("tab.posts"),
                count: posts.length,
                content:
                  posts.length === 0 ? (
                    <EmptyState glyph="✎" title={t("noPostsTitle")} body={t("noPostsBody")} />
                  ) : (
                    <div className="flex flex-col">
                      {posts.map((post) => (
                        <FeedPostCard key={post.id} post={post} />
                      ))}
                    </div>
                  ),
              },
              {
                id: "blogs",
                label: t("tab.blogs"),
                count: blogs.length,
                content:
                  blogs.length === 0 ? (
                    <EmptyState glyph="◫" title={t("noBlogsTitle")} body={t("noBlogsBody")} />
                  ) : (
                    <BlogDirectoryGridClient blogs={blogs} />
                  ),
              },
              {
                id: "comments",
                label: t("tab.comments"),
                count: comments.length,
                content:
                  comments.length === 0 ? (
                    <EmptyState glyph="❝" title={t("noCommentsTitle")} body={t("noCommentsBody")} />
                  ) : (
                    <ul className="flex flex-col rounded-xl border border-border bg-surface">
                      {comments.map((c) => (
                        <li key={c.id} className="flex flex-col gap-1 border-b border-border px-4 py-3 last:border-0">
                          <p className="text-[15px] leading-relaxed text-foreground">“{c.content}”</p>
                          <span className="text-[13px] text-muted">
                            {t("on")}{" "}
                            <Link href={c.permalink} className="text-foreground no-underline hover:underline">
                              {c.post_title}
                            </Link>{" "}
                            · {formatDate(c.created_at, locale, { day: "numeric", month: "short", year: "numeric" })}
                          </span>
                        </li>
                      ))}
                    </ul>
                  ),
              },
            ]}
          />

          <p className="text-[13px] leading-relaxed text-muted">{t("privacyNote")}</p>
        </div>
      </main>
    </>
  );
}
