import { getTranslations } from "next-intl/server";
import Image from "next/image";
import Link from "next/link";

import { VerificationBadge } from "@/components/ui/VerificationBadge";
import { interestLabel, interestsByKey } from "@/lib/interests";
import type { Interest, PublicUser } from "@/lib/types";

const hue = (s: string) => `oklch(0.55 0.06 ${[...s].reduce((a, c) => a + c.charCodeAt(0), 0) % 360})`;

function Avatar({ user, size = 44 }: { user: PublicUser; size?: number }) {
  const name = user.display_name ?? user.username;
  if (user.avatar_url) {
    return (
      <Image
        src={user.avatar_url}
        alt={name}
        width={size}
        height={size}
        className="shrink-0 rounded-full object-cover"
        style={{ width: size, height: size }}
        unoptimized
      />
    );
  }
  return (
    <span
      className="grid shrink-0 place-items-center rounded-full font-serif text-white"
      style={{ width: size, height: size, background: hue(user.username), fontSize: size * 0.43 }}
      aria-hidden="true"
    >
      {name[0]?.toUpperCase()}
    </span>
  );
}

/** Griglia di card per /users (directory pubblica degli utenti), stesso
 * schema di `BlogDirectoryGrid` per /blogs. */
export async function UserDirectoryGrid({
  users,
  interests = [],
  locale,
}: {
  users: PublicUser[];
  /** Elenco corrente (blocco "interessi utente"), per risolvere le chiavi
   * di `PublicUser.interests` in etichette leggibili sulle card. */
  interests?: Interest[];
  locale: string;
}) {
  const t = await getTranslations("UserDirectory");
  if (!users.length) return <p className="py-10 text-center text-sm text-muted">{t("noMatch")}</p>;
  const interestsMap = interestsByKey(interests);
  return (
    <div className="grid gap-5 sm:grid-cols-2 lg:grid-cols-3">
      {users.map((u) => (
        <article key={u.username} className="flex min-w-0 flex-col gap-3 rounded-[14px] border border-border bg-surface p-[22px]">
          <div className="flex items-center gap-3">
            <Avatar user={u} />
            <div className="flex min-w-0 flex-col leading-tight">
              <Link
                href={`/u/${u.username}`}
                className="flex items-center gap-1.5 truncate font-serif text-[19px] text-foreground no-underline hover:text-primary"
              >
                <span className="truncate">{u.display_name ?? u.username}</span>
                <VerificationBadge tier={u.verification_tier} size={15} />
              </Link>
              <span className="font-mono text-xs text-muted">@{u.custom_domain ?? u.username}</span>
            </div>
          </div>
          {u.bio && <p className="text-sm leading-relaxed text-muted line-clamp-3">{u.bio}</p>}
          {u.interests.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {u.interests.map((key) => (
                <Link
                  key={key}
                  href={`/users?interest=${encodeURIComponent(key)}`}
                  className="rounded-full border border-border px-2 py-0.5 text-[12px] text-muted no-underline hover:border-primary/40 hover:text-foreground"
                >
                  {interestLabel(interestsMap.get(key), key, locale)}
                </Link>
              ))}
            </div>
          )}
          <div className="mt-auto flex items-center justify-between pt-1.5 text-[13px] text-muted">
            <span>{t("followers", { count: u.follower_count })}</span>
            <Link href={`/u/${u.username}`} className="font-medium no-underline hover:underline">
              {t("follow")}
            </Link>
          </div>
        </article>
      ))}
    </div>
  );
}
