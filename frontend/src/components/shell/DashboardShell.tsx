"use client";
import Link from "next/link";
import { usePathname } from "next/navigation";
import type { ReactNode } from "react";

export interface NavItem {
  href: string;
  label: string;
  badge?: string | number;
  icon?: string;
  mobile?: boolean;
}

/**
 * Sidebar (≥lg, 240px) + tab bar in fondo (<lg, voci con mobile: true, max 5).
 * Usato sia da /dashboard sia da /admin — passare un `items`/`eyebrow` diverso.
 */
export function DashboardShell({
  items,
  eyebrow,
  homeHref = "/dashboard",
  footer,
  children,
}: {
  items: NavItem[];
  eyebrow?: string;
  homeHref?: string;
  footer?: ReactNode;
  children: ReactNode;
}) {
  const path = usePathname();
  const active = (href: string) => (href === "/dashboard" || href === "/admin" ? path === href : path.startsWith(href));
  const tabs = items.filter((i) => i.mobile).slice(0, 5);
  return (
    <div className="flex min-h-screen lg:grid lg:h-screen lg:grid-cols-[240px_minmax(0,1fr)] lg:overflow-hidden">
      <aside className="hidden border-r border-border bg-surface px-3.5 py-5 text-sm lg:flex lg:h-screen lg:flex-col lg:gap-1 lg:overflow-y-auto">
        <Link href={homeHref} className="flex flex-col px-3 pb-4 pt-1 leading-tight">
          <span className="font-serif text-xl font-semibold text-foreground">Notturni</span>
          {eyebrow && <span className="font-mono text-xs tracking-[.06em] text-muted">{eyebrow}</span>}
        </Link>
        {items.map((i) => (
          <Link
            key={i.href}
            href={i.href}
            className={`flex items-center justify-between rounded-lg px-3 py-2 ${
              active(i.href) ? "bg-primary/12 font-semibold text-foreground" : "text-muted hover:text-foreground"
            }`}
          >
            <span>{i.label}</span>
            {i.badge !== undefined && <span className="text-xs text-muted">{i.badge}</span>}
          </Link>
        ))}
        {footer && <div className="mt-auto border-t border-border pt-4">{footer}</div>}
      </aside>
      <main className="min-w-0 flex-1 px-5 pb-28 pt-6 lg:h-screen lg:overflow-y-auto lg:px-10 lg:py-9 lg:pb-9">{children}</main>
      {tabs.length > 0 && (
        <nav
          className="fixed inset-x-0 bottom-0 z-40 grid border-t border-border bg-surface px-2 pb-[max(env(safe-area-inset-bottom),1rem)] pt-2.5 text-[11px] lg:hidden"
          style={{ gridTemplateColumns: `repeat(${tabs.length}, 1fr)` }}
        >
          {tabs.map((tab) => (
            <Link
              key={tab.href}
              href={tab.href}
              className={`flex flex-col items-center gap-1 ${active(tab.href) ? "font-semibold text-primary" : "text-muted"}`}
            >
              <span className="text-lg leading-none" aria-hidden="true">
                {tab.icon}
              </span>
              {tab.label}
            </Link>
          ))}
        </nav>
      )}
    </div>
  );
}
