"use client";
import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useState, type CSSProperties, type ReactNode } from "react";

export interface NavItem {
  href: string;
  label: string;
  badge?: string | number;
  icon?: string;
  mobile?: boolean;
}

const COLLAPSE_STORAGE_KEY = "notturni_dashboard_sidebar_collapsed";

/**
 * Sidebar (≥lg, 240px, collassabile a 64px per lasciare più spazio al
 * contenuto — es. l'editor) + tab bar in fondo (<lg, voci con mobile: true,
 * max 5). Usato sia da /dashboard sia da /admin — passare un `items`/`eyebrow`
 * diverso. `lg:sticky lg:top-0 lg:h-screen lg:self-start` (invece di
 * stretch, il default della griglia) tiene l'aside fissata all'altezza della
 * finestra invece di allungarsi fino in fondo al contenuto centrale su
 * pagine lunghe (es. il profilo).
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

  const [collapsed, setCollapsed] = useState(false);
  // idratazione da localStorage: non disponibile lato server, va per forza
  // letta qui e non con uno stato iniziale "lazy" (vedi lib/theme-context.tsx).
  useEffect(() => {
    if (localStorage.getItem(COLLAPSE_STORAGE_KEY) === "1") {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setCollapsed(true);
    }
  }, []);
  function toggleCollapsed() {
    setCollapsed((prev) => {
      const next = !prev;
      localStorage.setItem(COLLAPSE_STORAGE_KEY, next ? "1" : "0");
      return next;
    });
  }

  return (
    <div
      className="flex min-h-screen lg:grid lg:grid-cols-[var(--sidebar-w)_minmax(0,1fr)]"
      style={{ "--sidebar-w": collapsed ? "68px" : "240px" } as CSSProperties}
    >
      <aside className="hidden overflow-y-auto border-r border-border bg-surface px-3.5 py-5 text-sm lg:sticky lg:top-0 lg:flex lg:h-screen lg:flex-col lg:gap-1 lg:self-start">
        <Link href={homeHref} className="flex flex-col px-3 pb-4 pt-1 leading-tight">
          <span className="font-serif text-xl font-semibold text-foreground">{collapsed ? "N" : "Notturni"}</span>
          {!collapsed && eyebrow && <span className="font-mono text-xs tracking-[.06em] text-muted">{eyebrow}</span>}
        </Link>
        {items.map((i) => (
          <Link
            key={i.href}
            href={i.href}
            title={collapsed ? i.label : undefined}
            className={`flex items-center gap-2.5 rounded-lg px-3 py-2 ${collapsed ? "justify-center" : "justify-between"} ${
              active(i.href) ? "bg-primary/12 font-semibold text-foreground" : "text-muted hover:text-foreground"
            }`}
          >
            {collapsed ? (
              <span className="text-base" aria-hidden="true">
                {i.icon ?? i.label[0]}
              </span>
            ) : (
              <>
                <span>{i.label}</span>
                {i.badge !== undefined && <span className="text-xs text-muted">{i.badge}</span>}
              </>
            )}
          </Link>
        ))}
        <div className="mt-auto flex flex-col gap-3">
          {footer && !collapsed && <div className="border-t border-border pt-4">{footer}</div>}
          <button
            type="button"
            onClick={toggleCollapsed}
            title={collapsed ? "Espandi" : "Comprimi"}
            className="flex items-center justify-center rounded-lg py-2 text-muted hover:bg-foreground/5 hover:text-foreground"
          >
            <span aria-hidden="true">{collapsed ? "»" : "« Comprimi"}</span>
          </button>
        </div>
      </aside>
      <main className="min-w-0 flex-1 px-5 pb-28 pt-6 lg:px-10 lg:py-9 lg:pb-9">{children}</main>
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
