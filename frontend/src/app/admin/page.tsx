"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useEffect, useState } from "react";

import { Card } from "@/components/ui/Card";
import { SkeletonRows } from "@/components/ui/States";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { PLATFORM_ADMIN_ROLES } from "@/lib/types";

interface Queues {
  pendingComments: number;
  hiddenPosts: number;
  suspendedBlogs: number;
  reviewPosts: number;
}

/** Panoramica admin (mockup 1g/5d): code aperte calcolate dagli endpoint
 * admin esistenti e scorciatoie alle sezioni. KPI di piattaforma, stato dei
 * servizi e "audit di oggi" arrivano con `GET /admin/overview` (blocco B1/B5). */
export default function AdminHomePage() {
  const { user, authFetch } = useAuth();
  const t = useTranslations("AdminHome");
  const tn = useTranslations("Nav");
  const isAdmin = !!user && PLATFORM_ADMIN_ROLES.includes(user.platform_role);
  const [queues, setQueues] = useState<Queues | null>(null);

  useEffect(() => {
    if (!user) return;
    Promise.all([
      authFetch((token) => api.admin.listComments(token, { status: "pending" })).catch(() => []),
      isAdmin ? authFetch((token) => api.admin.listPosts(token)).catch(() => []) : Promise.resolve([]),
      isAdmin ? authFetch((token) => api.admin.listBlogs(token)).catch(() => []) : Promise.resolve([]),
    ]).then(([comments, posts, blogs]) =>
      setQueues({
        pendingComments: comments.length,
        hiddenPosts: posts.filter((p) => p.is_hidden).length,
        reviewPosts: posts.filter((p) => p.status === "pending_review").length,
        suspendedBlogs: blogs.filter((b) => b.is_suspended).length,
      })
    );
  }, [user, isAdmin, authFetch]);

  const sections = [
    { href: "/admin/utenti", label: tn("users"), description: t("usersDesc"), admin: true },
    { href: "/admin/blog", label: tn("allBlogs"), description: t("blogsDesc"), admin: true },
    { href: "/admin/moderazione", label: tn("moderation"), description: t("moderationDesc"), admin: true },
    { href: "/admin/moderazione-commenti", label: tn("commentModeration"), description: t("commentsDesc"), admin: false },
    { href: "/admin/pagine", label: tn("staticPages"), description: t("pagesDesc"), admin: true },
    { href: "/admin/registro", label: tn("auditRegister"), description: t("auditDesc"), admin: true },
  ].filter((s) => isAdmin || !s.admin);

  const queueRows = queues
    ? [
        { label: t("queue.comments"), value: queues.pendingComments, href: "/admin/moderazione-commenti" },
        ...(isAdmin
          ? [
              { label: t("queue.review"), value: queues.reviewPosts, href: "/admin/moderazione" },
              { label: t("queue.hidden"), value: queues.hiddenPosts, href: "/admin/moderazione" },
              { label: t("queue.suspended"), value: queues.suspendedBlogs, href: "/admin/blog" },
            ]
          : []),
      ]
    : [];

  return (
    <div className="mx-auto flex max-w-5xl flex-col gap-8">
      <header className="flex flex-col gap-1">
        <h1 className="font-serif text-[28px] font-medium leading-tight text-foreground">{t("title")}</h1>
        <p className="text-sm text-muted">{t("signedInAs", { username: user?.username ?? "", role: user?.platform_role ?? "" })}</p>
      </header>

      <section className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_320px]">
        <Card className="flex flex-col gap-3">
          <div className="flex items-baseline justify-between">
            <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("openQueues")}</span>
          </div>
          {queues === null ? (
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
        <Card className="flex flex-col gap-2 text-sm">
          <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("services")}</span>
          <p className="text-[13px] leading-relaxed text-muted">{t("servicesNote")}</p>
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
