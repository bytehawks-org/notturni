"use client";

import { createContext, useContext } from "react";

import { setClientConfig, DEFAULT_SITE_NAME, type SiteConfig } from "@/lib/site";

const SiteConfigContext = createContext<SiteConfig | null>(null);

/** Rende disponibile al client la configurazione risolta a runtime sul server (vedi `lib/site.ts`).
 * `setClientConfig` avviene nel render, prima dei figli, così anche il primo render/idratazione e le
 * `fetch` negli effetti vedono URL giusti; il nome passa dal context perché può dipendere dalla lingua
 * (impostazioni di `/admin/seo`) e deve coincidere tra SSR e idratazione. */
export function SiteConfigProvider({ config, children }: { config: SiteConfig; children: React.ReactNode }) {
  setClientConfig(config);
  return <SiteConfigContext.Provider value={config}>{children}</SiteConfigContext.Provider>;
}

/** Nome della piattaforma da mostrare nell'interfaccia (client component). */
export function useSiteName(): string {
  return useContext(SiteConfigContext)?.siteName ?? DEFAULT_SITE_NAME;
}
