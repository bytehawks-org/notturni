"use client";

import { useTranslations } from "next-intl";
import { usePathname, useRouter } from "next/navigation";
import { useEffect } from "react";

import { DashboardShell, type NavItem } from "@/components/shell/DashboardShell";
import { UiLanguagePicker } from "@/components/shell/UiLanguagePicker";
import { ThemeToggle } from "@/components/ThemeToggle";
import { Button } from "@/components/ui/Button";
import { useAuth } from "@/lib/auth-context";
import { PLATFORM_ADMIN_ROLES, PLATFORM_MODERATION_ROLES } from "@/lib/types";

// Slug del blog su cui si sta lavorando, quando la pagina corrente è
// annidata sotto /dashboard/blogs/{slug}/... — usato solo per mostrare il
// link "vedi il blog" in DashboardShell, nessun altro effetto sul routing.
const BLOG_CONTEXT_RE = /^\/dashboard\/blogs\/([^/]+)/;

export default function DashboardLayout({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const pathname = usePathname();
  const { user, loading, logout } = useAuth();
  const t = useTranslations("Nav");
  const tc = useTranslations("Common");
  const currentBlogSlug = BLOG_CONTEXT_RE.exec(pathname)?.[1];

  useEffect(() => {
    if (!loading && !user) {
      router.replace("/login");
    }
  }, [loading, user, router]);

  if (loading || !user) {
    return (
      <main className="flex flex-1 items-center justify-center">
        <p className="text-sm text-muted">{tc("loading")}</p>
      </main>
    );
  }

  const isAdmin = PLATFORM_ADMIN_ROLES.includes(user.platform_role);
  const isModerator = PLATFORM_MODERATION_ROLES.includes(user.platform_role);

  const items: NavItem[] = [
    { href: "/dashboard", label: t("myBlogs"), icon: "✎", mobile: true },
    { href: "/dashboard/post", label: t("posts"), icon: "▤", mobile: true },
    { href: "/dashboard/media", label: t("media"), icon: "▣", mobile: true },
    { href: "/dashboard/pubblicazioni", label: t("publications"), icon: "▥", mobile: true },
    { href: "/dashboard/link", label: t("links"), icon: "⛓", mobile: true },
    { href: "/dashboard/bibliografia", label: t("bibliography"), icon: "❦", mobile: true },
    { href: "/dashboard/frammenti", label: t("fragments"), icon: "❝", mobile: true },
    { href: "/dashboard/profile", label: t("profile"), icon: "◯", mobile: true },
    { href: "/dashboard/token", label: t("apiTokens"), icon: "◈", mobile: true },
    ...(isAdmin || isModerator ? [{ href: "/admin", label: t("administrationShort"), icon: "⚙", mobile: true } as NavItem] : []),
  ];

  return (
    <div className="flex flex-1 flex-col">
      {/* Solo mobile: la sidebar con account/logout è nascosta sotto lg,
          e la tab bar del DashboardShell non lascia spazio a un footer —
          senza questa barra Esci sarebbe irraggiungibile su schermi piccoli. */}
      <div className="flex items-center justify-between border-b border-border px-5 py-3 text-sm lg:hidden">
        <span className="truncate text-muted">{user.username}</span>
        <div className="flex items-center gap-3">
          <ThemeToggle />
          <Button variant="secondary" size="sm" onClick={() => logout().then(() => router.push("/login"))}>
            {tc("signOut")}
          </Button>
        </div>
      </div>
      <DashboardShell
        items={items}
        platformLink={{ href: "/", label: t("backToPlatform") }}
        blogLink={currentBlogSlug ? { href: `/${currentBlogSlug}`, label: t("viewCurrentBlog", { slug: currentBlogSlug }) } : undefined}
        footer={(collapsed) =>
          collapsed ? (
            <div className="flex flex-col items-center gap-2 px-1">
              <div className="flex flex-wrap justify-center gap-1">
                <ThemeToggle />
              </div>
              <UiLanguagePicker className="w-full" />
              <button
                type="button"
                onClick={() => logout().then(() => router.push("/login"))}
                title={tc("signOut")}
                aria-label={tc("signOut")}
                className="flex h-8 w-8 items-center justify-center rounded-lg text-muted hover:bg-foreground/5 hover:text-foreground"
              >
                <span aria-hidden="true">⏻</span>
              </button>
            </div>
          ) : (
            <div className="flex flex-col gap-3 px-3 text-sm">
              <div className="flex items-center justify-between">
                <span className="truncate text-muted">{user.username}</span>
                <ThemeToggle />
              </div>
              <UiLanguagePicker className="w-full" />
              <Button variant="secondary" size="sm" onClick={() => logout().then(() => router.push("/login"))}>
                {tc("signOut")}
              </Button>
            </div>
          )
        }
      >
        {children}
      </DashboardShell>
    </div>
  );
}
