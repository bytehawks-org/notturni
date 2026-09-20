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
export interface ExternalLink {
  href: string;
  label: string;
  /** Icona compatta mostrata a sidebar collassata (un solo carattere, come `NavItem.icon`). */
  icon?: string;
}

export function DashboardShell({
  items,
  eyebrow,
  homeHref = "/dashboard",
  /** Link verso la piattaforma generale (homepage pubblica) — sempre
   * presente, non legato al contesto della pagina corrente. */
  platformLink,
  /** Link verso il blog pubblico su cui si sta lavorando: valorizzato solo
   * dalle pagine che operano nel contesto di un blog specifico (es.
   * l'editor di un post), assente altrove — permette di capire subito quale
   * blog è "attivo" senza doverlo dedurre dallo slug nell'URL. */
  blogLink,
  footer,
  children,
}: {
  items: NavItem[];
  eyebrow?: string;
  homeHref?: string;
  platformLink?: ExternalLink;
  blogLink?: ExternalLink;
  /** Sia il nodo espanso sia un compattatore `(collapsed) => ReactNode`: il
   * footer porta azioni sempre raggiungibili (logout, tema) — con il solo
   * `ReactNode` sparivano del tutto a sidebar collassata, non solo
   * ridimensionate. */
  footer?: ReactNode | ((collapsed: boolean) => ReactNode);
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
        {(platformLink || blogLink) && (
          <div className={`mb-2 flex flex-col gap-1 ${collapsed ? "items-center" : ""}`}>
            {platformLink && (
              <Link
                href={platformLink.href}
                title={collapsed ? platformLink.label : undefined}
                className={`flex items-center gap-2 rounded-lg px-3 py-1.5 text-[13px] text-muted hover:text-foreground ${collapsed ? "justify-center" : ""}`}
              >
                <span aria-hidden="true">{platformLink.icon ?? "↩"}</span>
                {!collapsed && <span className="truncate">{platformLink.label}</span>}
              </Link>
            )}
            {blogLink && (
              <Link
                href={blogLink.href}
                title={collapsed ? blogLink.label : undefined}
                className={`flex items-center gap-2 rounded-lg px-3 py-1.5 text-[13px] text-muted hover:text-foreground ${collapsed ? "justify-center" : ""}`}
              >
                <span aria-hidden="true">{blogLink.icon ?? "↗"}</span>
                {!collapsed && <span className="truncate">{blogLink.label}</span>}
              </Link>
            )}
          </div>
        )}
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
          {footer && (
            <div className={collapsed ? "" : "border-t border-border pt-4"}>
              {typeof footer === "function" ? footer(collapsed) : !collapsed && footer}
            </div>
          )}
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
