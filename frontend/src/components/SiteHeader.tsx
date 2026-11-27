"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { useState } from "react";

import { useSiteFlags, useSiteName } from "@/components/SiteConfigProvider";
import { SearchModal } from "@/components/SearchModal";
import { PlatformMark } from "@/components/shell/PlatformMark";
import { UiLanguagePicker } from "@/components/shell/UiLanguagePicker";
import { ThemeToggle } from "@/components/ThemeToggle";
import { Button } from "@/components/ui/Button";
import { useAuth } from "@/lib/auth-context";
import { usePrimaryBlogSlug } from "@/lib/primary-blog-client";
import type { SearchScope } from "@/lib/search";

const PORTAL_SCOPE: SearchScope = { kind: "portal" };

/** Ricerca in solo: l'ambito è il blog primario (`basePath` vuoto). Montato solo quando il modal è aperto,
 * così lo slug è letto (o richiesto, se manca nel provider) solo all'apertura. Senza slug: ambito piattaforma. */
function SoloSearch({ onClose }: { onClose: () => void }) {
  const slug = usePrimaryBlogSlug();
  const scope: SearchScope = slug ? { kind: "blog", slug, basePath: "" } : PORTAL_SCOPE;
  return <SearchModal open onClose={onClose} scope={scope} />;
}

/** Header della piattaforma (mockup 4a/1c). "Pubblicazioni" arriverà con il blocco B9. */
export function SiteHeader() {
  const { user, loading } = useAuth();
  const path = usePathname();
  const t = useTranslations("Site");
  const siteName = useSiteName();
  const { isSolo } = useSiteFlags();
  const [searchOpen, setSearchOpen] = useState(false);
  const items: [string, string][] = [
    ["/", t("latest")],
    ["/blogs", t("blogs")],
    ["/users", t("people")],
  ];
  const onSearch = path === "/search";

  return (
    <header className="border-b border-border">
      <div className="mx-auto flex h-[60px] w-full max-w-[1184px] items-center justify-between px-5 lg:px-12">
        <div className="flex items-center gap-7">
          <div className="flex items-center gap-2.5">
            <PlatformMark />
            <Link href="/" className="font-serif text-[21px] font-semibold tracking-tight text-foreground no-underline">
              {siteName}
            </Link>
          </div>
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
          <button
            type="button"
            onClick={() => setSearchOpen(true)}
            aria-label={t("search")}
            title={t("search")}
            className={`flex h-8 w-8 items-center justify-center rounded-full text-xl leading-none ${onSearch ? "font-medium text-foreground" : "text-muted hover:bg-background hover:text-foreground"}`}
          >
            <span aria-hidden="true">⌕</span>
          </button>
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
      {searchOpen &&
        (isSolo ? (
          <SoloSearch onClose={() => setSearchOpen(false)} />
        ) : (
          <SearchModal open onClose={() => setSearchOpen(false)} scope={PORTAL_SCOPE} />
        ))}
    </header>
  );
}
