"use client";

import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

import { ThemeToggle } from "@/components/ThemeToggle";
import { Button } from "@/components/ui/Button";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { PLATFORM_ADMIN_ROLES, PLATFORM_MODERATION_ROLES } from "@/lib/types";

export default function AdminLayout({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const { user, loading, logout } = useAuth();
  const [deploymentMode, setDeploymentMode] = useState<"solo" | "platform" | null>(null);

  useEffect(() => {
    if (!loading && !user) {
      router.replace("/login");
    }
  }, [loading, user, router]);

  useEffect(() => {
    api.config
      .get()
      .then((config) => setDeploymentMode(config.deployment_mode))
      .catch(() => setDeploymentMode("platform"));
  }, []);

  const isAdmin = !!user && PLATFORM_ADMIN_ROLES.includes(user.platform_role);
  const isModerator = !!user && PLATFORM_MODERATION_ROLES.includes(user.platform_role);

  useEffect(() => {
    if (!loading && user && !isAdmin && !isModerator) {
      router.replace("/dashboard");
    }
  }, [loading, user, isAdmin, isModerator, router]);

  if (loading || !user || (!isAdmin && !isModerator)) {
    return (
      <main className="flex flex-1 items-center justify-center">
        <p className="text-sm text-muted">Caricamento…</p>
      </main>
    );
  }

  return (
    <div className="flex flex-1 flex-col">
      <header className="border-b border-border">
        <div className="mx-auto flex max-w-5xl items-center justify-between px-6 py-4">
          <nav className="flex items-center gap-6">
            <Link href="/admin" className="font-serif text-lg text-foreground">
              Amministrazione
            </Link>
            {isAdmin && (
              <Link href="/admin/pagine" className="text-sm text-muted hover:text-foreground">
                Pagine
              </Link>
            )}
            {isAdmin && deploymentMode !== "solo" && (
              <Link href="/admin/utenti" className="text-sm text-muted hover:text-foreground">
                Utenti
              </Link>
            )}
            {isAdmin && (
              <Link href="/admin/blog" className="text-sm text-muted hover:text-foreground">
                Tutti i blog
              </Link>
            )}
            {isAdmin && (
              <Link href="/admin/moderazione" className="text-sm text-muted hover:text-foreground">
                Moderazione
              </Link>
            )}
            {isModerator && (
              <Link href="/admin/moderazione-commenti" className="text-sm text-muted hover:text-foreground">
                Moderazione commenti
              </Link>
            )}
            {isAdmin && (
              <Link href="/admin/registro" className="text-sm text-muted hover:text-foreground">
                Registro
              </Link>
            )}
          </nav>
          <div className="flex items-center gap-4">
            <Link href="/dashboard" className="text-sm text-muted hover:text-foreground">
              ← La mia dashboard
            </Link>
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
