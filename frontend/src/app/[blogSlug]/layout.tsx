import type { Metadata } from "next";
import type { ReactNode } from "react";

import { getPublicBlog } from "@/lib/server-api";

interface PageParams {
  blogSlug: string;
}

/** Solo per i metadata: la favicon dedicata del blog (facoltativa,
 * SettingsTab) va applicata a tutte le pagine pubbliche del blog (home,
 * post, bibliografia, media, link, pagina, pubblicazioni) senza ripeterla
 * in ognuna — stesso principio di `u/[username]/layout.tsx`. Assente:
 * nessun override, resta il comportamento di piattaforma. */
export async function generateMetadata({
  params,
}: {
  params: Promise<PageParams>;
}): Promise<Metadata> {
  const { blogSlug } = await params;
  const blog = await getPublicBlog(blogSlug);
  if (!blog?.favicon_url) return {};
  return { icons: { icon: blog.favicon_url } };
}

export default function BlogLayout({ children }: { children: ReactNode }) {
  return children;
}
