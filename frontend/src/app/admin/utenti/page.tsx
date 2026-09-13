"use client";

import { useLocale, useTranslations } from "next-intl";
import { useCallback, useEffect, useState } from "react";

import { SearchInput } from "@/components/SearchInput";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Pill } from "@/components/ui/Pill";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import type { AdminUser, PlatformRole } from "@/lib/types";

const ROLES: PlatformRole[] = ["utente", "moderatore", "amministratore", "super_admin"];

/** Tabella utenti (mockup 1g): ricerca, ruolo, blog, MFA, ultimo accesso,
 * stato, export CSV (generato nel browser dall'elenco caricato). */
export default function DashboardUsersPage() {
  const { user: me, authFetch } = useAuth();
  const t = useTranslations("AdminUsers");
  const tc = useTranslations("Common");
  const locale = useLocale();
  const [q, setQ] = useState("");
  const [users, setUsers] = useState<AdminUser[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [rowError, setRowError] = useState<Record<string, string>>({});

  const errorMessage = useCallback(
    (err: unknown) => (err instanceof ApiClientError ? err.message : tc("unexpectedError")),
    [tc]
  );

  const load = useCallback(() => {
    authFetch((token) => api.admin.listUsers(token, q))
      .then(setUsers)
      .catch((err) => setError(errorMessage(err)));
  }, [authFetch, q, errorMessage]);

  useEffect(load, [load]);

  async function update(userId: string, payload: { platform_role?: PlatformRole; is_active?: boolean }) {
    setRowError((prev) => ({ ...prev, [userId]: "" }));
    try {
      const updated = await authFetch((token) => api.admin.updateUser(token, userId, payload));
      setUsers((prev) => prev?.map((u) => (u.id === userId ? updated : u)) ?? null);
    } catch (err) {
      setRowError((prev) => ({ ...prev, [userId]: errorMessage(err) }));
    }
  }

  const canGrantPrivilegedRoles = me?.platform_role === "super_admin";

  function exportCsv() {
    if (!users) return;
    const rows = [
      ["username", "email", "platform_role", "is_active", "mfa_enabled", "blogs_count", "created_at", "last_seen_at"],
      ...users.map((u) => [u.username, u.email, u.platform_role, u.is_active, u.mfa_enabled, u.blogs_count, u.created_at, u.last_seen_at ?? ""]),
    ];
    const csv = rows.map((r) => r.map((v) => `"${String(v).replace(/"/g, '""')}"`).join(",")).join("\n");
    const url = URL.createObjectURL(new Blob([csv], { type: "text/csv;charset=utf-8" }));
    const a = document.createElement("a");
    a.href = url;
    a.download = "notturni-utenti.csv";
    a.click();
    URL.revokeObjectURL(url);
  }

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-5">
      <div className="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
        <div className="flex flex-col gap-1">
          <h1 className="font-serif text-[28px] font-medium leading-tight text-foreground">{t("title")}</h1>
          {users && <p className="text-sm text-muted">{t("summary", { count: users.length })}</p>}
        </div>
        <div className="flex items-center gap-2">
          <SearchInput value={q} onChange={setQ} placeholder={t("search")} />
          <Button size="sm" variant="secondary" onClick={exportCsv} disabled={!users || users.length === 0}>
            {t("exportCsv")}
          </Button>
        </div>
      </div>
      {error && <Alert kind="error">{error}</Alert>}
      {users === null && !error && <SkeletonRows rows={6} />}
      {users !== null && users.length === 0 && <EmptyState glyph="◯" title={t("emptyTitle")} body={t("emptyBody")} />}
      {users !== null && users.length > 0 && (
        <div className="overflow-x-auto rounded-xl border border-border bg-surface">
          <table className="w-full text-left text-sm">
            <thead className="border-b border-border font-mono text-[11px] uppercase tracking-[.06em] text-muted">
              <tr>
                <th className="px-4 py-3">{t("col.user")}</th>
                <th className="px-4 py-3">{t("col.role")}</th>
                <th className="px-4 py-3">{t("col.blogs")}</th>
                <th className="px-4 py-3">{t("col.mfa")}</th>
                <th className="px-4 py-3">{t("col.since")}</th>
                <th className="px-4 py-3">{t("col.lastSeen")}</th>
                <th className="px-4 py-3">{t("col.status")}</th>
              </tr>
            </thead>
            <tbody>
              {users.map((u) => {
                const roleIsPrivileged = u.platform_role === "amministratore" || u.platform_role === "super_admin";
                const roleSelectDisabled = roleIsPrivileged && !canGrantPrivilegedRoles;
                return (
                  <tr key={u.id} className="border-b border-border last:border-0">
                    <td className="px-4 py-3">
                      <div className="flex flex-col">
                        <span className="font-medium text-foreground">@{u.username}</span>
                        <span className="text-xs text-muted">{u.email}</span>
                      </div>
                    </td>
                    <td className="px-4 py-3">
                      <select
                        value={u.platform_role}
                        disabled={roleSelectDisabled}
                        onChange={(e) => update(u.id, { platform_role: e.target.value as PlatformRole })}
                        className="rounded-md border border-border bg-background px-2 py-1 text-sm disabled:opacity-50"
                      >
                        {ROLES.map((role) => (
                          <option
                            key={role}
                            value={role}
                            disabled={(role === "amministratore" || role === "super_admin") && !canGrantPrivilegedRoles}
                          >
                            {t(`role.${role}`)}
                          </option>
                        ))}
                      </select>
                      {rowError[u.id] && <p className="mt-1 text-xs text-danger">{rowError[u.id]}</p>}
                    </td>
                    <td className="px-4 py-3 text-muted">{u.blogs_count}</td>
                    <td className="px-4 py-3">
                      <Pill tone={u.mfa_enabled ? "ok" : "neutral"}>{u.mfa_enabled ? t("mfaOn") : t("mfaOff")}</Pill>
                    </td>
                    <td className="whitespace-nowrap px-4 py-3 text-muted">
                      {formatDate(u.created_at, locale, { day: "numeric", month: "short", year: "numeric" })}
                    </td>
                    <td className="whitespace-nowrap px-4 py-3 text-muted">
                      {u.last_seen_at ? formatDate(u.last_seen_at, locale, { day: "numeric", month: "short", year: "numeric" }) : t("never")}
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex items-center gap-3">
                        <Pill tone={u.is_active ? "ok" : "danger"}>{u.is_active ? t("active") : t("inactive")}</Pill>
                        {u.id !== me?.id && (
                          <Button size="sm" variant="secondary" onClick={() => update(u.id, { is_active: !u.is_active })}>
                            {u.is_active ? t("deactivate") : t("activate")}
                          </Button>
                        )}
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
