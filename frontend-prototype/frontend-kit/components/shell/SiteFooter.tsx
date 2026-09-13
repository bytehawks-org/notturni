import { useTranslations } from "next-intl";
import Link from "next/link";

export function SiteFooter() {
  const t = useTranslations("Site");
  const links: [string, string][] = [[t("about"), "/p/about"], [t("contacts"), "/p/contatti"], [t("privacy"), "/p/privacy"], [t("terms"), "/p/termini"], [t("status"), "https://status.notturni.eu"]];
  return (
    <footer className="border-t border-border">
      <div className="mx-auto flex w-full max-w-[1184px] flex-col gap-4 px-5 py-7 text-[13px] text-muted md:flex-row md:items-center md:justify-between lg:px-12">
        <span className="font-serif text-[15px] text-foreground">Notturni</span>
        <nav className="flex flex-wrap gap-[22px]">{links.map(([l, h]) => <Link key={l} href={h} className="text-muted no-underline hover:text-foreground">{l}</Link>)}<a href="https://github.com/bytehawks-org/notturni" className="text-muted no-underline hover:text-foreground">{t("sourceCode")}</a></nav>
        <span>{t("madeInEu")}</span>
      </div>
    </footer>
  );
}
