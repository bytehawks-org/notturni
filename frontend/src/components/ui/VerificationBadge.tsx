import type { VerificationTier } from "@/lib/types";

/** Sigillo di verifica del profilo (stile Bluesky/Instagram/Twitter),
 * CLAUDE.md #5: quattro colori previsti, ma solo "bronze" ha oggi una logica
 * che lo assegna (dominio custom verificato via DNS) — silver/gold/blue
 * restano riservati per future integrazioni. "none" non renderizza nulla. */
const TIER_COLORS: Record<Exclude<VerificationTier, "none">, string> = {
  bronze: "#B08D57",
  silver: "#A8A9AD",
  gold: "#D4AF37",
  blue: "var(--primary, #2563eb)",
};

// Fallback italiano se il chiamante non passa `label` — i chiamanti reali
// (u/[username]/page.tsx, dashboard/profile/page.tsx) passano invece la voce
// localizzata VerificationTier.<tier> (next-intl), altrimenti gli utenti
// anglofoni sentirebbero/leggerebbero sempre l'etichetta italiana.
const DEFAULT_TIER_LABELS: Record<Exclude<VerificationTier, "none">, string> = {
  bronze: "Verificato — dominio personalizzato",
  silver: "Verificato — argento",
  gold: "Verificato — oro",
  blue: "Verificato — blu",
};

export function VerificationBadge({
  tier,
  size = 16,
  label,
}: {
  tier: VerificationTier;
  size?: number;
  label?: string;
}) {
  if (tier === "none") return null;

  const color = TIER_COLORS[tier];
  const resolvedLabel = label ?? DEFAULT_TIER_LABELS[tier];

  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill={color}
      role="img"
      aria-label={resolvedLabel}
      className="inline-block shrink-0 align-middle"
    >
      <title>{resolvedLabel}</title>
      <path d="M12 1.5l2.6 1.6 3-.4 1 2.9 2.7 1.5-.7 3 1.9 2.4-1.9 2.4.7 3-2.7 1.5-1 2.9-3-.4L12 22.5l-2.6-1.6-3 .4-1-2.9-2.7-1.5.7-3-1.9-2.4 1.9-2.4-.7-3 2.7-1.5 1-2.9 3 .4L12 1.5Z" />
      <path
        d="M8.2 12.3l2.6 2.6 5-5"
        fill="none"
        stroke="var(--surface, #fff)"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
