"use client";

import { useTranslations } from "next-intl";
import { useRouter } from "next/navigation";
import { useEffect } from "react";

import { DashboardShell, type NavItem } from "@/components/shell/DashboardShell";
import { ThemeToggle } from "@/components/ThemeToggle";
import { Button } from "@/components/ui/Button";
import { useAuth } from "@/lib/auth-context";
import { PLATFORM_ADMIN_ROLES, PLATFORM_MODERATION_ROLES } from "@/lib/types";

export default function DashboardLayout({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const { user, loading, logout } = useAuth();
  const t = useTranslations("Nav");
  const tc = useTranslations("Common");

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
        footer={
          <div className="flex flex-col gap-3 px-3 text-sm">
            <div className="flex items-center justify-between">
              <span className="truncate text-muted">{user.username}</span>
              <ThemeToggle />
            </div>
            <Button variant="secondary" size="sm" onClick={() => logout().then(() => router.push("/login"))}>
            {tc("signOut")}
          </Button>
          </div>
        }
      >
        {children}
      </DashboardShell>
    </div>
  );
}
