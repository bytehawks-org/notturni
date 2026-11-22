import type { Metadata, Viewport } from "next";
import { NextIntlClientProvider } from "next-intl";
import { getLocale, getMessages } from "next-intl/server";
import { headers } from "next/headers";
import Script from "next/script";

import { LightboxProvider } from "@/components/Lightbox";
import { CodeCopyProvider } from "@/components/blog/CodeCopyProvider";
import { ToastProvider } from "@/components/ui/Toast";
import { AuthProvider } from "@/lib/auth-context";
import { NonceProvider } from "@/lib/nonce-context";
import { getSiteSeo } from "@/lib/server-api";
import { SiteConfigProvider } from "@/components/SiteConfigProvider";
import { siteConfig, siteUrl } from "@/lib/site";
import { ThemeProvider } from "@/lib/theme-context";

import "./fonts.css";
import "./globals.css";

const DEFAULT_DESCRIPTION =
  "Piattaforma opensource di microblogging e newsletter, multilingua ed EU-centrica.";

/** `it` → `it_IT`, `en` → `en_US`: il formato che si aspetta Open Graph. */
const OG_LOCALES: Record<string, string> = { it: "it_IT", en: "en_US", de: "de_DE", fr: "fr_FR", es: "es_ES" };

/** Meta tag del portale, dalle impostazioni di `/admin/seo` (modulo `branding`) nella lingua del visitatore. Se il
 * modulo è spento o il backend non risponde restano i valori predefiniti. */
export async function generateMetadata(): Promise<Metadata> {
  const locale = await getLocale();
  const seo = await getSiteSeo(locale);
  const siteName = seo?.site_name || siteConfig().siteName;
  const description = seo?.description || DEFAULT_DESCRIPTION;
  const other: Record<string, string> = {};
  if (seo?.bing_verification) other["msvalidate.01"] = seo.bing_verification;
  for (const m of seo?.extra_meta ?? []) other[m.name] = m.content;
  const social = seo?.social_image_url ? [{ url: seo.social_image_url }] : undefined;
  return {
    // Risolve i percorsi relativi passati a `openGraph.images`/`alternates.canonical` dalle pagine figlie — senza
    // questo, Next li lascerebbe relativi anche nell'HTML finale (invalidi per i consumer esterni di Open Graph).
    metadataBase: new URL(siteUrl()),
    title: { default: seo?.title || siteName, template: `%s · ${siteName}` },
    description,
    keywords: seo?.keywords || undefined,
    applicationName: siteName,
    manifest: "/manifest.webmanifest",
    icons: seo && seo.icons.length > 0 ? { icon: seo.icons, apple: seo.apple_touch_icon ?? undefined } : undefined,
    robots: seo?.noindex ? { index: false, follow: false } : undefined,
    verification: {
      google: seo?.google_verification || undefined,
      yandex: seo?.yandex_verification || undefined,
      other: Object.keys(other).length > 0 ? other : undefined,
    },
    openGraph: { siteName, type: "website", locale: OG_LOCALES[locale] ?? locale, description, images: social },
    twitter: { card: social ? "summary_large_image" : "summary", site: seo?.twitter_site || undefined, images: social?.map((i) => i.url) },
  };
}

export async function generateViewport(): Promise<Viewport> {
  const seo = await getSiteSeo(await getLocale());
  return seo?.theme_color ? { themeColor: seo.theme_color } : {};
}

export default async function RootLayout({ children }: LayoutProps<"/">) {
  // Lingua dell'interfaccia (src/i18n/request.ts): cookie → Accept-Language → fallback.
  const locale = await getLocale();
  const messages = await getMessages();
  // Nome mostrato nell'interfaccia: `/admin/seo` (per lingua) → NOCT_SITE_NAME → "Notturni". URL del sito e
  // dell'API sono risolti a runtime (lib/site.ts), non inglobati nel bundle.
  const seo = await getSiteSeo(locale);
  const config = { ...siteConfig(), siteName: seo?.site_name || siteConfig().siteName };
  // Nonce CSP della richiesta corrente (proxy.ts): 'self' non basta per gli
  // script inline (né i nostri né quelli che Next stesso inietta per
  // l'idratazione), va passato esplicitamente a ogni <Script>.
  const nonce = (await headers()).get("x-nonce");
  return (
    <html
      lang={locale}
      className="h-full antialiased"
    >
      <body className="min-h-full flex flex-col bg-background text-foreground font-sans">
        {/* File esterno (public/theme-init.js), non inline — comunque
            soggetto a script-src, serve il nonce. */}
        <Script src="/theme-init.js" strategy="beforeInteractive" nonce={nonce ?? undefined} />
        <NonceProvider nonce={nonce}>
          <NextIntlClientProvider locale={locale} messages={messages}>
            <SiteConfigProvider config={config}>
            <ThemeProvider>
              <AuthProvider>
                <ToastProvider>
                  <LightboxProvider>{children}</LightboxProvider>
                  <CodeCopyProvider />
                </ToastProvider>
              </AuthProvider>
            </ThemeProvider>
            </SiteConfigProvider>
          </NextIntlClientProvider>
        </NonceProvider>
      </body>
    </html>
  );
}
