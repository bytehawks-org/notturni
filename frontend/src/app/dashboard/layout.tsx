"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect } from "react";

import { ThemeToggle } from "@/components/ThemeToggle";
import { Button } from "@/components/ui/Button";
import { useAuth } from "@/lib/auth-context";
import { PLATFORM_ADMIN_ROLES } from "@/lib/types";

export default function DashboardLayout({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const { user, loading, logout } = useAuth();

  useEffect(() => {
    if (!loading && !user) {
      router.replace("/login");
    }
  }, [loading, user, router]);

  if (loading || !user) {
    return (
      <main className="flex flex-1 items-center justify-center">
        <p className="text-sm text-muted">Caricamento…</p>
      </main>
    );
  }

  const isAdmin = PLATFORM_ADMIN_ROLES.includes(user.platform_role);

  return (
    <div className="flex flex-1 flex-col">
      <header className="border-b border-border">
        <div className="mx-auto flex max-w-5xl items-center justify-between px-6 py-4">
          <nav className="flex items-center gap-6">
            <Link href="/dashboard" className="font-serif text-lg text-foreground">
              Notturni
            </Link>
            <Link href="/dashboard" className="text-sm text-muted hover:text-foreground">
              I miei blog
            </Link>
            <Link href="/dashboard/profile" className="text-sm text-muted hover:text-foreground">
              Profilo
            </Link>
            {isAdmin && (
              <Link href="/admin" className="text-sm text-muted hover:text-foreground">
                Amministrazione
              </Link>
            )}
          </nav>
          <div className="flex items-center gap-4">
            <ThemeToggle />
            <span className="text-sm text-muted">{user.username}</span>
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
