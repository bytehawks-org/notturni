import Link from "next/link";

import { SITE_URL } from "@/lib/site";

/** Marchio minimale della piattaforma: una "N" in un riquadro colorato, per
 * tornare alla home del portale da qualunque punto — in particolare dalla
 * top bar di un blog (BlogHeader), che altrimenti non ha alcun link verso
 * `notturni.eu`. URL assoluto (non `/`, relativo): quando arriverà il
 * routing per sottodominio-per-blog (ROADMAP.md §3), `/` da un blog
 * risolverebbe sul blog stesso, non sulla piattaforma — `SITE_URL` (da
 * `NEXT_PUBLIC_SITE_URL`, la env dedicata al FQDN) resta corretto in
 * entrambi i casi. Nessuna direttiva "use client": va bene sia nella
 * SiteHeader (client) sia nella BlogHeader (Server Component). */
export function PlatformMark({ className = "" }: { className?: string }) {
  return (
    <Link
      href={SITE_URL}
      aria-label="Notturni"
      title="Notturni"
      className={`grid h-8 w-8 shrink-0 place-items-center rounded-lg bg-primary font-serif text-[15px] font-semibold text-background no-underline ${className}`}
    >
      N
    </Link>
  );
}
