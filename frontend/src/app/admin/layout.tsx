"use client";

import Link from "next/link";
import { useTranslations } from "next-intl";
import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";

import { DashboardShell, type NavItem } from "@/components/shell/DashboardShell";
import { ThemeToggle } from "@/components/ThemeToggle";
import { Button } from "@/components/ui/Button";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { PLATFORM_ADMIN_ROLES, PLATFORM_MODERATION_ROLES } from "@/lib/types";

export default function AdminLayout({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const { user, loading, logout } = useAuth();
  const t = useTranslations("Nav");
  const tc = useTranslations("Common");
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
        <p className="text-sm text-muted">{tc("loading")}</p>
      </main>
    );
  }

  const items: NavItem[] = [
    ...(isAdmin ? [{ href: "/admin/pagine", label: t("pages"), icon: "▤", mobile: true } as NavItem] : []),
    ...(isAdmin && deploymentMode !== "solo"
      ? [{ href: "/admin/utenti", label: t("users"), icon: "◯", mobile: true } as NavItem]
      : []),
    ...(isAdmin ? [{ href: "/admin/blog", label: t("allBlogs"), icon: "✎", mobile: true } as NavItem] : []),
    ...(isAdmin ? [{ href: "/admin/moderazione", label: t("moderation"), icon: "◔", mobile: true } as NavItem] : []),
    ...(isModerator
      ? [{ href: "/admin/moderazione-commenti", label: t("commentModeration"), icon: "◔", mobile: true } as NavItem]
      : []),
    ...(isAdmin ? [{ href: "/admin/registro", label: t("register"), icon: "≡", mobile: true } as NavItem] : []),
  ];

  return (
    <div className="flex flex-1 flex-col">
      <div className="flex items-center justify-between border-b border-border px-5 py-3 text-sm lg:hidden">
        <Link href="/dashboard" className="text-muted hover:text-foreground">
          {tc("myDashboard")}
        </Link>
        <div className="flex items-center gap-3">
          <ThemeToggle />
          <Button variant="secondary" size="sm" onClick={() => logout().then(() => router.push("/login"))}>
            {tc("signOut")}
          </Button>
        </div>
      </div>
      <DashboardShell
        items={items}
        eyebrow={t("administration")}
        homeHref="/admin"
        footer={
          <div className="flex flex-col gap-3 px-3 text-sm">
            <Link href="/dashboard" className="text-muted hover:text-foreground">
              {tc("myDashboard")}
            </Link>
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
