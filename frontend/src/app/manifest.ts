import type { MetadataRoute } from "next";
import { getLocale } from "next-intl/server";

import { getSiteSeo } from "@/lib/server-api";
import { siteName } from "@/lib/site";

export const dynamic = "force-dynamic";

/** Web app manifest: nome, colore e icone del portale (quelle caricate in `/admin/seo`, se ci sono). */
export default async function manifest(): Promise<MetadataRoute.Manifest> {
  const seo = await getSiteSeo(await getLocale());
  const name = seo?.site_name || siteName();
  return {
    name,
    short_name: name.length > 12 ? name.slice(0, 12) : name,
    description: seo?.description || undefined,
    start_url: "/",
    display: "browser",
    background_color: "#ffffff",
    theme_color: seo?.theme_color || undefined,
    icons: (seo?.icons ?? []).map((i) => ({ src: i.url, sizes: i.sizes, type: i.type })),
  };
}
