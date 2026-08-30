"use client";

import Link from "next/link";

import { ThemeToggle } from "@/components/ThemeToggle";
import { useAuth } from "@/lib/auth-context";

export function SiteHeader() {
  const { user, loading } = useAuth();

  return (
    <header className="border-b border-border">
      <div className="mx-auto flex max-w-5xl items-center justify-between px-6 py-4">
        <Link href="/" className="font-serif text-lg text-foreground">
          Notturni
        </Link>
        <div className="flex items-center gap-4">
          <ThemeToggle />
          {!loading && user ? (
            <Link href="/dashboard" className="text-sm text-primary underline underline-offset-4">
              Dashboard
            </Link>
          ) : (
            !loading && (
              <>
                <Link href="/login" className="text-sm text-muted hover:text-foreground">
                  Accedi
                </Link>
                <Link href="/register" className="text-sm text-primary underline underline-offset-4">
                  Registrati
                </Link>
              </>
            )
          )}
        </div>
      </div>
    </header>
  );
}
