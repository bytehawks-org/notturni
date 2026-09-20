import { useTranslations } from "next-intl";

import { API_URL } from "@/lib/api";

const PROVIDER_LABELS: Record<string, string> = {
  google: "Google",
  microsoft: "Microsoft",
  github: "GitHub",
  linkedin: "LinkedIn",
};

/** Un link per provider (redirect a `GET /auth/sso/{provider}/login`, mai
 * una fetch: il flow OAuth2/OIDC richiede una navigazione vera del browser,
 * non una chiamata XHR) — mostrati solo se configurati sull'istanza e
 * abilitati a livello di piattaforma (`GET /api/v1/config::sso_providers`,
 * già l'intersezione dei due, vedi backend/app/api/v1/health.py). Assente
 * (array vuoto) su un'istanza senza alcun provider SSO: login/registrazione
 * restano solo via email/password, nessun placeholder mostrato. */
export function SsoButtons({ providers }: { providers: string[] }) {
  const t = useTranslations("Auth");
  if (providers.length === 0) return null;

  return (
    <div className="flex flex-col gap-3.5">
      <div className="flex items-center gap-3 text-[13px] text-muted">
        <span className="h-px flex-1 bg-border" />
        {t("ssoDivider")}
        <span className="h-px flex-1 bg-border" />
      </div>
      <div className="flex flex-col gap-2">
        {providers.map((provider) => (
          <a
            key={provider}
            href={`${API_URL}/api/v1/auth/sso/${provider}/login`}
            className="inline-flex items-center justify-center gap-2 rounded-lg border border-border px-3.5 py-2 text-sm font-medium text-foreground no-underline transition hover:bg-foreground/5"
          >
            {t("ssoContinueWith", { provider: PROVIDER_LABELS[provider] ?? provider })}
          </a>
        ))}
      </div>
    </div>
  );
}
