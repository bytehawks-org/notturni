"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect } from "react";

import { ThemeToggle } from "@/components/ThemeToggle";
import { Button } from "@/components/ui/Button";
import { useAuth } from "@/lib/auth-context";
import { PLATFORM_ADMIN_ROLES } from "@/lib/types";

export default function AdminLayout({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const { user, loading, logout } = useAuth();

  const isAdmin = user ? PLATFORM_ADMIN_ROLES.includes(user.platform_role) : false;

  useEffect(() => {
    if (loading) return;
    if (!user) {
      router.replace("/login");
    } else if (!isAdmin) {
      router.replace("/dashboard");
    }
  }, [loading, user, isAdmin, router]);

  if (loading || !user || !isAdmin) {
    return (
      <main className="flex flex-1 items-center justify-center">
        <p className="text-sm text-muted">Caricamento…</p>
      </main>
    );
  }

  return (
    <div className="flex flex-1 flex-col">
      <header className="border-b border-border bg-foreground/5">
        <div className="mx-auto flex max-w-5xl items-center justify-between px-6 py-4">
          <nav className="flex items-center gap-6">
            <Link href="/admin" className="font-serif text-lg text-foreground">
              Amministrazione
            </Link>
            <Link href="/admin/pages" className="text-sm text-muted hover:text-foreground">
              Pagine
            </Link>
            <Link href="/admin/users" className="text-sm text-muted hover:text-foreground">
              Utenti
            </Link>
            <Link href="/dashboard" className="text-sm text-muted hover:text-foreground">
              Torna alla dashboard
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
