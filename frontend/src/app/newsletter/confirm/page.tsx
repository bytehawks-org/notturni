import type { Metadata } from "next";
import { getTranslations } from "next-intl/server";
import Link from "next/link";

import { SiteHeader } from "@/components/SiteHeader";
import { SiteFooter } from "@/components/shell/SiteFooter";
import { API_URL } from "@/lib/api";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("NewsletterConfirmPage");
  return { title: t("title"), robots: { index: false, follow: false } };
}

/** Conferma dell'iscrizione (link cliccato dall'email di doppio opt-in):
 * chiamata diretta all'API in fase di render server, nessuna interazione
 * richiesta — a differenza della disiscrizione (sempre un POST esplicito,
 * mai un side-effect legato al solo caricamento del link). */
export default async function NewsletterConfirmPage({
  searchParams,
}: {
  searchParams: Promise<{ token?: string }>;
}) {
  const { token } = await searchParams;
  const t = await getTranslations("NewsletterConfirmPage");

  let status: "confirmed" | "already_confirmed" | "invalid" = "invalid";
  if (token) {
    try {
      const res = await fetch(`${API_URL}/api/v1/newsletter/confirm?token=${encodeURIComponent(token)}`, {
        cache: "no-store",
      });
      if (res.ok) {
        const data = (await res.json()) as { status: "confirmed" | "already_confirmed" | "invalid" };
        status = data.status;
      }
    } catch {
      status = "invalid";
    }
  }

  const copy = {
    confirmed: { title: t("confirmedTitle"), body: t("confirmedBody") },
    already_confirmed: { title: t("alreadyTitle"), body: t("alreadyBody") },
    invalid: { title: t("invalidTitle"), body: t("invalidBody") },
  }[status];

  return (
    <>
      <SiteHeader />
      <main className="mx-auto flex w-full max-w-lg flex-1 flex-col items-center justify-center gap-4 px-5 py-20 text-center">
        <h1 className="font-serif text-3xl font-medium text-foreground">{copy.title}</h1>
        <p className="text-[15px] leading-relaxed text-muted">{copy.body}</p>
        <Link href="/" className="text-sm font-medium text-primary no-underline hover:underline">
          {t("backHome")}
        </Link>
      </main>
      <SiteFooter />
    </>
  );
}
