"use client";

import Link from "next/link";

import { ThemeToggle } from "@/components/ThemeToggle";
import { Button } from "@/components/ui/Button";
import { usePathname } from "next/navigation";

import { useAuth } from "@/lib/auth-context";

export function SiteHeader() {
  const { user, loading } = useAuth();
  const path = usePathname();

  return (
    <header className="border-b border-border">
      <div className="mx-auto flex h-[60px] w-full max-w-[1184px] items-center justify-between px-5 lg:px-12">
        <Link href="/" className="font-serif text-[21px] font-semibold tracking-tight text-foreground no-underline">
          Notturni
        </Link>
        <nav className="hidden gap-[26px] text-sm text-muted md:flex">
          <Link href="/" className={`no-underline ${path === "/" ? "font-medium text-foreground" : "hover:text-foreground"}`}>
            Ultimi articoli
          </Link>
          <Link
            href="/blogs"
            className={`no-underline ${path === "/blogs" ? "font-medium text-foreground" : "hover:text-foreground"}`}
          >
            Blog
          </Link>
        </nav>
        <div className="flex items-center gap-4 text-sm">
          <span className="hidden sm:inline-flex">
            <ThemeToggle />
          </span>
          {!loading &&
            (user ? (
              <Link href="/dashboard">
                <Button size="sm">Dashboard</Button>
              </Link>
            ) : (
              <>
                <Link href="/login" className="text-muted no-underline hover:text-foreground">
                  Accedi
                </Link>
                <Link href="/register">
                  <Button size="sm">Crea account</Button>
                </Link>
              </>
            ))}
        </div>
      </div>
    </header>
  );
}
