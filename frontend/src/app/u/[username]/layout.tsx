import type { Metadata } from "next";
import type { ReactNode } from "react";

import { SiteFooter } from "@/components/shell/SiteFooter";
import { getPublicUserProfile } from "@/lib/server-api";

interface PageParams {
  username: string;
}

/** Metadata del profilo pubblico (mockup 3e). La stessa chiamata a
 * `getPublicUserProfile` viene rifatta anche in `page.tsx`: Next.js
 * deduplica i `fetch` con lo stesso URL/opzioni entro la stessa richiesta,
 * quindi non è una seconda round-trip verso il backend. */
export async function generateMetadata({
  params,
}: {
  params: Promise<PageParams>;
}): Promise<Metadata> {
  const { username } = await params;
  const profile = await getPublicUserProfile(username);
  if (!profile) return {};
  const title = profile.display_name ?? `@${profile.username}`;
  const description = profile.bio
    ? profile.bio.length > 160
      ? `${profile.bio.slice(0, 160).trimEnd()}…`
      : profile.bio
    : undefined;
  return {
    title,
    description,
    alternates: { canonical: `/u/${username}` },
    openGraph: { title, description, type: "profile", url: `/u/${username}` },
  };
}

export default function UserProfileLayout({ children }: { children: ReactNode }) {
  return (
    <>
      {children}
      <SiteFooter />
    </>
  );
}
