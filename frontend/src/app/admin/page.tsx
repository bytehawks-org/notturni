"use client";

import Link from "next/link";

import { Card, CardTitle } from "@/components/ui/Card";
import { useAuth } from "@/lib/auth-context";
import { PLATFORM_ADMIN_ROLES } from "@/lib/types";

const ADMIN_SECTIONS = [
  { href: "/admin/pagine", label: "Pagine statiche", description: "Editor delle pagine del sito principale." },
  { href: "/admin/utenti", label: "Utenti", description: "Ruoli e attivazione degli account." },
  { href: "/admin/blog", label: "Tutti i blog", description: "Elenco di piattaforma, sospensione." },
  { href: "/admin/moderazione", label: "Moderazione", description: "Nascondi/mostra i post di ogni blog." },
  {
    href: "/admin/moderazione-commenti",
    label: "Moderazione commenti",
    description: "Commenti in attesa su tutti i blog.",
  },
  { href: "/admin/registro", label: "Registro di audit", description: "Cronologia delle azioni sensibili." },
] as const;

export default function AdminHomePage() {
  const { user } = useAuth();
  const isAdmin = !!user && PLATFORM_ADMIN_ROLES.includes(user.platform_role);

  const sections = ADMIN_SECTIONS.filter((s) => isAdmin || s.href === "/admin/moderazione-commenti");

  return (
    <div>
      <h1 className="mb-6 font-serif text-2xl text-foreground">Pannello di amministrazione</h1>
      <div className="grid gap-4 sm:grid-cols-2">
        {sections.map((s) => (
          <Link key={s.href} href={s.href}>
            <Card className="h-full transition hover:border-primary">
              <CardTitle>{s.label}</CardTitle>
              <p className="text-sm text-muted">{s.description}</p>
            </Card>
          </Link>
        ))}
      </div>
    </div>
  );
}
