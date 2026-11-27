"use client";

import { createContext, useContext } from "react";

import { setClientConfig, DEFAULT_SITE_NAME, type SiteConfig } from "@/lib/site";

/** Configurazione risolta sul server, più lo slug del blog primario (solo; `null` altrove). */
export type SiteConfigValue = SiteConfig & { primarySlug: string | null };

const SiteConfigContext = createContext<SiteConfigValue | null>(null);

/** Rende disponibile al client la configurazione risolta a runtime sul server (vedi `lib/site.ts`).
 * `setClientConfig` avviene nel render, prima dei figli, così anche il primo render/idratazione e le
 * `fetch` negli effetti vedono URL giusti; il nome passa dal context perché può dipendere dalla lingua
 * (impostazioni di `/admin/seo`) e deve coincidere tra SSR e idratazione. Lo slug del primario arriva
 * dal server (`getPrimaryBlogSlug()` nel layout), così il primo render SSR lo conosce già. */
export function SiteConfigProvider({
  config,
  primarySlug = null,
  children,
}: {
  config: SiteConfig;
  primarySlug?: string | null;
  children: React.ReactNode;
}) {
  setClientConfig(config);
  const value: SiteConfigValue = { ...config, primarySlug };
  return <SiteConfigContext.Provider value={value}>{children}</SiteConfigContext.Provider>;
}

/** Configurazione dal provider, o `null` se il componente non è sotto un `SiteConfigProvider`. */
export function useSiteConfigValue(): SiteConfigValue | null {
  return useContext(SiteConfigContext);
}

/** Nome della piattaforma da mostrare nell'interfaccia (client component). */
export function useSiteName(): string {
  return useContext(SiteConfigContext)?.siteName ?? DEFAULT_SITE_NAME;
}

/** Flag di deployment (solo/platform, sottodomini) per i client component. Senza provider: valori predefiniti
 * della piattaforma (comportamento invariato). */
export function useSiteFlags(): { isSolo: boolean; subdomainsEnabled: boolean } {
  const config = useContext(SiteConfigContext);
  return { isSolo: config?.deploymentMode === "solo", subdomainsEnabled: config?.subdomainsEnabled ?? true };
}
