"use client";

import Link from "next/link";

import { useSiteName } from "@/components/SiteConfigProvider";
import { siteUrl } from "@/lib/site";

/** Marchio minimale della piattaforma: l'iniziale del nome in un riquadro colorato, per tornare alla home
 * del portale da qualunque punto — in particolare dalla top bar di un blog (BlogHeader), che altrimenti non
 * ha alcun link verso la piattaforma. URL assoluto (non `/`, relativo): con il routing per
 * sottodominio-per-blog (ROADMAP.md §3) `/` da un blog risolverebbe sul blog stesso, non sulla
 * piattaforma — `siteUrl()` (runtime, vedi lib/site.ts) resta corretto in entrambi i casi. Client
 * component perché il nome arriva dal context (`/admin/seo` o `NOCT_SITE_NAME`). */
export function PlatformMark({ className = "" }: { className?: string }) {
  const name = useSiteName();
  return (
    <Link
      href={siteUrl()}
      aria-label={name}
      title={name}
      className={`grid h-8 w-8 shrink-0 place-items-center rounded-lg bg-primary font-serif text-[15px] font-semibold text-background no-underline ${className}`}
    >
      {name.charAt(0).toUpperCase()}
    </Link>
  );
}
