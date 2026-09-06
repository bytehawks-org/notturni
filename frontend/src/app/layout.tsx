import type { Metadata } from "next";
import { Inter, Lora } from "next/font/google";
import Script from "next/script";

import { AuthProvider } from "@/lib/auth-context";
import { SITE_URL } from "@/lib/site";
import { ThemeProvider } from "@/lib/theme-context";

import "./globals.css";

const bodyFont = Inter({
  variable: "--font-body",
  subsets: ["latin"],
});

const headingFont = Lora({
  variable: "--font-heading",
  subsets: ["latin"],
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

// Applica subito il tema salvato (o la preferenza di sistema come primissima
// stima) prima che React idrati, per evitare un flash del tema sbagliato.
// ThemeProvider corregge poi con il calcolo alba/tramonto se la modalità è "auto".
const THEME_INIT_SCRIPT = `
(function () {
  try {
    var stored = localStorage.getItem('notturni_theme_mode');
    var resolved = (stored === 'light' || stored === 'dark')
      ? stored
      : (window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
    document.documentElement.setAttribute('data-theme', resolved);
  } catch (e) {}
})();
`;

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html
      lang="it"
      className={`${bodyFont.variable} ${headingFont.variable} h-full antialiased`}
    >
      <body className="min-h-full flex flex-col bg-background text-foreground font-sans">
        <Script id="theme-init" strategy="beforeInteractive">
          {THEME_INIT_SCRIPT}
        </Script>
        <ThemeProvider>
          <AuthProvider>{children}</AuthProvider>
        </ThemeProvider>
      </body>
    </html>
  );
}
