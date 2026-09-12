import Link from "next/link";

// Footer minimale: i link a pagine statiche (chi siamo/contatti/privacy/...)
// non sono qui perché sono contenuto libero creato dall'Amministratore su
// /p/{slug} — nessuno slug è riservato/garantito, quindi non si può
// linkarle a colpo sicuro da un componente condiviso senza dati reali
// (vedi todo/UX_REDESIGN.md).
export function SiteFooter() {
  return (
    <footer className="border-t border-border">
      <div className="mx-auto flex w-full max-w-[1184px] flex-col gap-4 px-5 py-7 text-[13px] text-muted md:flex-row md:items-center md:justify-between lg:px-12">
        <span className="font-serif text-[15px] text-foreground">Notturni</span>
        <nav className="flex flex-wrap gap-[22px]">
          <Link href="https://github.com/bytehawks-org/notturni" className="text-muted no-underline hover:text-foreground">
            Codice sorgente
          </Link>
        </nav>
      </div>
    </footer>
  );
}
