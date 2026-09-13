import { useTranslations } from "next-intl";
import Link from "next/link";

// Footer (mockup 4a): i link a pagine statiche (chi siamo/contatti/privacy/...)
// non sono qui perché sono contenuto libero creato dall'Amministratore su
// /p/{slug} — nessuno slug è riservato/garantito, quindi non si possono
// linkare a colpo sicuro senza dati reali (vedi todo/UX_REDESIGN.md).
export function SiteFooter() {
  const t = useTranslations("Site");
  return (
    <footer className="border-t border-border">
      <div className="mx-auto flex w-full max-w-[1184px] flex-col gap-4 px-5 py-7 text-[13px] text-muted md:flex-row md:items-center md:justify-between lg:px-12">
        <span className="font-serif text-[15px] text-foreground">Notturni</span>
        <nav className="flex flex-wrap items-center gap-[22px]">
          <Link href="https://github.com/bytehawks-org/notturni" className="text-muted no-underline hover:text-foreground">
            {t("sourceCode")}
          </Link>
          <span>{t("madeInEu")}</span>
        </nav>
      </div>
    </footer>
  );
}
