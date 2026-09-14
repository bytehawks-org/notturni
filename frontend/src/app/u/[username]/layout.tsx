import type { Metadata } from "next";
import type { ReactNode } from "react";

import { getPublicUserProfile } from "@/lib/server-api";

interface PageParams {
  username: string;
}

/** Solo per i metadata (mockup 3e): `/u/{username}/page.tsx` è un Client
 * Component (dati interattivi — follow, tab post/blog/commenti — richiedono
 * la sessione dell'utente, mai disponibile a un Server Component), quindi
 * non può esportare `generateMetadata`; un `layout.tsx` Server Component
 * nello stesso segmento può farlo al suo posto senza toccare la pagina. */
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
  return children;
}
