"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useEffect, useState } from "react";

import { Card } from "@/components/ui/Card";
import { Pill } from "@/components/ui/Pill";
import { SkeletonRows } from "@/components/ui/States";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { PLATFORM_ADMIN_ROLES, type AdminOverview } from "@/lib/types";

/** Panoramica admin (mockup 1g/5d): KPI, code aperte, audit di oggi e
 * salute dei servizi da `GET /admin/overview`, più le scorciatoie alle sezioni. */
export default function AdminHomePage() {
  const { user, authFetch } = useAuth();
  const t = useTranslations("AdminHome");
  const tn = useTranslations("Nav");
  const isAdmin = !!user && PLATFORM_ADMIN_ROLES.includes(user.platform_role);
  const [overview, setOverview] = useState<AdminOverview | null>(null);

  useEffect(() => {
    if (!user) return;
    authFetch((token) => api.admin.overview(token))
      .then(setOverview)
      .catch(() => undefined);
  }, [user, authFetch]);

  const sections = [
    { href: "/admin/utenti", label: tn("users"), description: t("usersDesc"), admin: true },
    { href: "/admin/blog", label: tn("allBlogs"), description: t("blogsDesc"), admin: true },
    { href: "/admin/moderazione", label: tn("moderation"), description: t("moderationDesc"), admin: true },
    { href: "/admin/moderazione-commenti", label: tn("commentModeration"), description: t("commentsDesc"), admin: false },
    { href: "/admin/pagine", label: tn("staticPages"), description: t("pagesDesc"), admin: true },
    { href: "/admin/registro", label: tn("auditRegister"), description: t("auditDesc"), admin: true },
  ].filter((s) => isAdmin || !s.admin);

  const queueRows = overview
    ? [
        { label: t("queue.comments"), value: overview.queue_pending_comments, href: "/admin/moderazione-commenti" },
        { label: t("queue.review"), value: overview.queue_posts_in_review, href: "/admin/moderazione" },
        { label: t("queue.hidden"), value: overview.queue_hidden_posts, href: "/admin/moderazione" },
        { label: t("queue.suspended"), value: overview.blogs_suspended, href: "/admin/blog" },
      ]
    : [];
  const allOk = overview?.services.every((s) => s.status !== "down") ?? true;

  return (
    <div className="mx-auto flex max-w-5xl flex-col gap-8">
      <header className="flex flex-col gap-1">
        <h1 className="font-serif text-[28px] font-medium leading-tight text-foreground">{t("title")}</h1>
        <p className="text-sm text-muted">
          {t("signedInAs", { username: user?.username ?? "", role: user?.platform_role ?? "" })}
          {overview && (
            <>
              {" · "}
              <span className={allOk ? "text-ok" : "text-danger"}>● {allOk ? t("operational") : t("degraded")}</span>
              {" · "}
              {t("mode", { mode: overview.deployment_mode })}
            </>
          )}
        </p>
      </header>

      {overview && (
        <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
          {[
            [overview.users_total, t("kpi.users"), t("kpi.usersNew", { count: overview.users_new_7d })],
            [overview.blogs_total, t("kpi.blogs"), t("kpi.blogsSuspended", { count: overview.blogs_suspended })],
            [overview.posts_published, t("kpi.posts"), ""],
            [overview.audit_today, t("kpi.audit"), ""],
          ].map(([value, label, sub]) => (
            <div key={String(label)} className="flex flex-col gap-1 rounded-xl border border-border bg-surface px-4 py-3.5">
              <span className="font-serif text-2xl text-foreground">{value}</span>
              <span className="text-xs text-muted">{label}</span>
              {sub && <span className="text-[11px] text-muted">{sub}</span>}
            </div>
          ))}
        </div>
      )}

      <section className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_320px]">
        <Card className="flex flex-col gap-3">
          <div className="flex items-baseline justify-between">
            <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("openQueues")}</span>
          </div>
          {overview === null ? (
            <SkeletonRows rows={3} />
          ) : (
            <ul className="flex flex-col">
              {queueRows.map((row) => (
                <li key={row.label} className="flex items-center justify-between border-b border-border py-2.5 text-sm last:border-0">
                  <span className="text-foreground">{row.label}</span>
                  <span className="flex items-center gap-3">
                    <span className={`font-serif text-lg ${row.value > 0 ? "text-foreground" : "text-muted"}`}>{row.value}</span>
                    <Link href={row.href} className="text-[13px] text-primary no-underline hover:underline">
                      {t("open")}
                    </Link>
                  </span>
                </li>
              ))}
            </ul>
          )}
        </Card>
        <Card className="flex flex-col gap-3 text-sm">
          <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("services")}</span>
          {overview === null ? (
            <SkeletonRows rows={3} />
          ) : (
            <ul className="flex flex-col">
              {overview.services.map((svc) => (
                <li key={svc.name} className="flex items-center justify-between border-b border-border py-2 last:border-0">
                  <span className="font-mono text-[13px] text-foreground">{svc.name}</span>
                  <Pill tone={svc.status === "ok" ? "ok" : svc.status === "down" ? "danger" : "neutral"}>
                    {t(`service.${svc.status}`)}
                  </Pill>
                </li>
              ))}
            </ul>
          )}
        </Card>
      </section>

      <section className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {sections.map((s) => (
          <Link key={s.href} href={s.href} className="no-underline">
            <Card className="h-full transition hover:border-primary">
              <span className="font-serif text-[17px] text-foreground">{s.label}</span>
              <p className="mt-1 text-[13px] text-muted">{s.description}</p>
            </Card>
          </Link>
        ))}
      </section>
    </div>
  );
}
