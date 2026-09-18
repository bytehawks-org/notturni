import type { Metadata } from "next";
import { Lora, Source_Sans_3 } from "next/font/google";
import { NextIntlClientProvider } from "next-intl";
import { getLocale, getMessages } from "next-intl/server";
import Script from "next/script";

import { LightboxProvider } from "@/components/Lightbox";
import { ToastProvider } from "@/components/ui/Toast";
import { AuthProvider } from "@/lib/auth-context";
import { SITE_URL } from "@/lib/site";
import { ThemeProvider } from "@/lib/theme-context";

import "./globals.css";

const bodyFont = Source_Sans_3({
  variable: "--font-body",
  subsets: ["latin", "latin-ext"],
  weight: ["400", "500", "600"],
});

const headingFont = Lora({
  variable: "--font-heading",
  subsets: ["latin", "latin-ext"],
  weight: ["400", "500", "600"],
  style: ["normal", "italic"],
});

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
  return (
    <html
      lang={locale}
      className={`${bodyFont.variable} ${headingFont.variable} h-full antialiased`}
    >
      <body className="min-h-full flex flex-col bg-background text-foreground font-sans">
        {/* File esterno (public/theme-init.js), non inline — vedi la nota lì
            sulla CSP script-src 'self' (k8s/middleware-security-headers.yaml). */}
        <Script src="/theme-init.js" strategy="beforeInteractive" />
        <NextIntlClientProvider locale={locale} messages={messages}>
          <ThemeProvider>
            <AuthProvider>
              <ToastProvider>
                <LightboxProvider>{children}</LightboxProvider>
              </ToastProvider>
            </AuthProvider>
          </ThemeProvider>
        </NextIntlClientProvider>
      </body>
    </html>
  );
}
