"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { usePathname } from "next/navigation";

import { UiLanguagePicker } from "@/components/shell/UiLanguagePicker";
import { ThemeToggle } from "@/components/ThemeToggle";
import { Button } from "@/components/ui/Button";
import { useAuth } from "@/lib/auth-context";

/** Header della piattaforma (mockup 4a/1c). "Pubblicazioni" arriverà con il blocco B9. */
export function SiteHeader() {
  const { user, loading } = useAuth();
  const path = usePathname();
  const t = useTranslations("Site");
  const items: [string, string][] = [
    ["/", t("latest")],
    ["/blogs", t("blogs")],
  ];

  return (
    <header className="border-b border-border">
      <div className="mx-auto flex h-[60px] w-full max-w-[1184px] items-center justify-between px-5 lg:px-12">
        <div className="flex items-center gap-7">
          <Link href="/" className="font-serif text-[21px] font-semibold tracking-tight text-foreground no-underline">
            Notturni
          </Link>
          <nav className="hidden gap-[26px] text-sm text-muted md:flex">
            {items.map(([href, label]) => (
              <Link
                key={href}
                href={href}
                className={`no-underline ${path === href ? "font-medium text-foreground" : "hover:text-foreground"}`}
              >
                {label}
              </Link>
            ))}
          </nav>
        </div>
        <div className="flex items-center gap-3 text-sm">
          <span className="hidden sm:inline-flex">
            <ThemeToggle />
          </span>
          <span className="hidden md:inline-flex">
            <UiLanguagePicker />
          </span>
          {!loading &&
            (user ? (
              <Link href="/dashboard">
                <Button size="sm">{t("dashboard")}</Button>
              </Link>
            ) : (
              <>
                <Link href="/login" className="text-muted no-underline hover:text-foreground">
                  {t("signIn")}
                </Link>
                <Link href="/register">
                  <Button size="sm">{t("createAccount")}</Button>
                </Link>
              </>
            ))}
        </div>
      </div>
    </header>
  );
}
