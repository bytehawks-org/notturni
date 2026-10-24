import type { Metadata } from "next";
import { NextIntlClientProvider } from "next-intl";
import { getLocale, getMessages } from "next-intl/server";
import { headers } from "next/headers";
import Script from "next/script";

import { LightboxProvider } from "@/components/Lightbox";
import { CodeCopyProvider } from "@/components/blog/CodeCopyProvider";
import { ToastProvider } from "@/components/ui/Toast";
import { AuthProvider } from "@/lib/auth-context";
import { NonceProvider } from "@/lib/nonce-context";
import { SITE_URL } from "@/lib/site";
import { ThemeProvider } from "@/lib/theme-context";

import "./fonts.css";
import "./globals.css";

const DEFAULT_DESCRIPTION =
  "Piattaforma opensource di microblogging e newsletter, multilingua ed EU-centrica.";

export const metadata: Metadata = {
  // Risolve i percorsi relativi passati a `openGraph.images`/`alternates.canonical`
  // dalle pagine figlie — senza questo, Next li lascerebbe relativi anche nell'HTML
  // finale (invalidi per i consumer esterni di Open Graph/canonical).
  metadataBase: new URL(SITE_URL),
  title: { default: "Notturni", template: "%s · Notturni" },
  description: DEFAULT_DESCRIPTION,
  openGraph: {
    siteName: "Notturni",
    type: "website",
    locale: "it_IT",
    description: DEFAULT_DESCRIPTION,
  },
  twitter: {
    card: "summary_large_image",
  },
};

export default async function RootLayout({ children }: LayoutProps<"/">) {
  // Lingua dell'interfaccia (src/i18n/request.ts): cookie → Accept-Language → fallback.
  const locale = await getLocale();
  const messages = await getMessages();
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
            <ThemeProvider>
              <AuthProvider>
                <ToastProvider>
                  <LightboxProvider>{children}</LightboxProvider>
                  <CodeCopyProvider />
                </ToastProvider>
              </AuthProvider>
            </ThemeProvider>
          </NextIntlClientProvider>
        </NonceProvider>
      </body>
    </html>
  );
}
