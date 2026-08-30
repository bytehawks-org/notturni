import { SiteHeader } from "@/components/SiteHeader";

export default function Home() {
  return (
    <>
      <SiteHeader />
      <main className="flex-1 flex items-center justify-center px-6 py-24">
        <div className="max-w-xl text-center space-y-6">
          <h1 className="font-serif text-4xl sm:text-5xl tracking-tight">Notturni</h1>
          <p className="text-lg text-muted leading-relaxed">
            Permettere all&apos;utente di esprimersi nella forma più naturale possibile: la
            parola. Nella sicurezza e nella ricchezza della propria lingua.
          </p>
        </div>
      </main>
    </>
  );
}
