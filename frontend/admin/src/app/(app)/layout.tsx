"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

import { ThemeToggle } from "@/components/ThemeToggle";
import { Button } from "@/components/ui/Button";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { PLATFORM_ADMIN_ROLES } from "@/lib/types";

export default function AppLayout({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const { user, loading, logout } = useAuth();
  const [deploymentMode, setDeploymentMode] = useState<"solo" | "platform" | null>(null);

  const isAdmin = user ? PLATFORM_ADMIN_ROLES.includes(user.platform_role) : false;

  useEffect(() => {
    if (loading) return;
    if (!user) {
      router.replace("/login");
    } else if (!isAdmin) {
      router.replace("/login");
    }
  }, [loading, user, isAdmin, router]);

  useEffect(() => {
    api.config
      .get()
      .then((config) => setDeploymentMode(config.deployment_mode))
      .catch(() => setDeploymentMode("platform"));
  }, []);

  if (loading || !user || !isAdmin) {
    return (
      <main className="flex flex-1 items-center justify-center">
        <p className="text-sm text-muted">Caricamento…</p>
      </main>
    );
  }

  // In modalità "solo" c'è un solo utente (già Super Admin): la sezione
  // Utenti (ruoli/attivazione di più account) non ha senso da mostrare.
  const showUsersSection = deploymentMode !== "solo";

  return (
    <div className="flex flex-1 flex-col">
      <header className="border-b border-border bg-foreground/5">
        <div className="mx-auto flex max-w-5xl items-center justify-between px-6 py-4">
          <nav className="flex items-center gap-6">
            <Link href="/" className="font-serif text-lg text-foreground">
              Amministrazione
            </Link>
            <Link href="/pagine" className="text-sm text-muted hover:text-foreground">
              Pagine
            </Link>
            {showUsersSection && (
              <Link href="/utenti" className="text-sm text-muted hover:text-foreground">
                Utenti
              </Link>
            )}
            <Link href="/blog" className="text-sm text-muted hover:text-foreground">
              Blog
            </Link>
          </nav>
          <div className="flex items-center gap-4">
            <ThemeToggle />
            <span className="text-sm text-muted">
              {user.username} ({user.platform_role})
            </span>
            <Button variant="secondary" onClick={() => logout().then(() => router.push("/login"))}>
              Esci
            </Button>
          </div>
        </div>
      </header>
      <main className="mx-auto w-full max-w-5xl flex-1 px-6 py-8">{children}</main>
    </div>
  );
}
