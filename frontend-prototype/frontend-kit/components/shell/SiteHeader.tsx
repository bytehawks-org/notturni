"use client";
import { useTranslations } from "next-intl";
import Link from "next/link";
import { ThemeToggle } from "@/components/ThemeToggle";
import { useAuth } from "@/lib/auth-context";
import { Button } from "../ui/Button";

const NAV: [string, string][] = [["latest", "/"], ["blogs", "/blogs"], ["publications", "/publications"], ["about", "/p/about"]];

export function SiteHeader({ current }: { current?: string }) {
  const t = useTranslations("Site");
  const { user, loading } = useAuth();
  return (
    <header className="border-b border-border">
      <div className="mx-auto flex h-[60px] w-full max-w-[1184px] items-center justify-between px-5 lg:px-12">
        <Link href="/" className="font-serif text-[21px] font-semibold tracking-tight text-foreground no-underline">Notturni</Link>
        <nav className="hidden gap-[26px] text-sm text-muted md:flex">
          {NAV.map(([k, href]) => <Link key={k} href={href} className={`no-underline ${k === current ? "font-medium text-foreground" : "hover:text-foreground"}`}>{t(k)}</Link>)}
          <a href="https://blog.notturni.eu" className="no-underline hover:text-foreground">blog.notturni.eu ↗</a>
        </nav>
        <div className="flex items-center gap-4 text-sm">
          <span className="hidden sm:inline-flex"><ThemeToggle /></span>
          {!loading && (user
            ? <Link href="/dashboard"><Button size="sm">{t("dashboard")}</Button></Link>
            : <><Link href="/login" className="text-muted no-underline hover:text-foreground">{t("signIn")}</Link><Link href="/register"><Button size="sm">{t("createAccount")}</Button></Link></>)}
        </div>
      </div>
    </header>
  );
}
