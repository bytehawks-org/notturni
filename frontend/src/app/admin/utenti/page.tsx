"use client";

import { useCallback, useEffect, useState } from "react";

import { SearchInput } from "@/components/SearchInput";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { AdminUser, PlatformRole } from "@/lib/types";

function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

const ROLES: PlatformRole[] = ["utente", "moderatore", "amministratore", "super_admin"];

export default function DashboardUsersPage() {
  const { user: me, authFetch } = useAuth();
  const [q, setQ] = useState("");
  const [users, setUsers] = useState<AdminUser[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [rowError, setRowError] = useState<Record<string, string>>({});

  const load = useCallback(() => {
    authFetch((token) => api.admin.listUsers(token, q))
      .then(setUsers)
      .catch((err) => setError(errorMessage(err)));
  }, [authFetch, q]);

  useEffect(load, [load]);

  async function handleRoleChange(userId: string, platform_role: PlatformRole) {
    setRowError((prev) => ({ ...prev, [userId]: "" }));
    try {
      const updated = await authFetch((token) => api.admin.updateUser(token, userId, { platform_role }));
      setUsers((prev) => prev?.map((u) => (u.id === userId ? updated : u)) ?? null);
    } catch (err) {
      setRowError((prev) => ({ ...prev, [userId]: errorMessage(err) }));
    }
  }

  async function handleToggleActive(userId: string, is_active: boolean) {
    setRowError((prev) => ({ ...prev, [userId]: "" }));
    try {
      const updated = await authFetch((token) => api.admin.updateUser(token, userId, { is_active }));
      setUsers((prev) => prev?.map((u) => (u.id === userId ? updated : u)) ?? null);
    } catch (err) {
      setRowError((prev) => ({ ...prev, [userId]: errorMessage(err) }));
    }
  }

  const canGrantPrivilegedRoles = me?.platform_role === "super_admin";

  return (
    <div>
      <div className="mb-6 flex items-center justify-between">
        <h1 className="font-serif text-2xl text-foreground">Utenti</h1>
        <SearchInput value={q} onChange={setQ} placeholder="Cerca per username o email…" />
      </div>
      {error && <Alert kind="error">{error}</Alert>}

      <div className="overflow-x-auto rounded-lg border border-border">
        <table className="w-full text-left text-sm">
          <thead className="border-b border-border text-muted">
            <tr>
              <th className="px-4 py-3">Username</th>
              <th className="px-4 py-3">Email</th>
              <th className="px-4 py-3">Ruolo</th>
              <th className="px-4 py-3">Stato</th>
            </tr>
          </thead>
          <tbody>
            {users?.map((u) => {
              const roleIsPrivileged = u.platform_role === "amministratore" || u.platform_role === "super_admin";
              const roleSelectDisabled = (roleIsPrivileged || false) && !canGrantPrivilegedRoles;
              return (
                <tr key={u.id} className="border-b border-border last:border-0">
                  <td className="px-4 py-3 text-foreground">{u.username}</td>
                  <td className="px-4 py-3 text-muted">{u.email}</td>
                  <td className="px-4 py-3">
                    <select
                      value={u.platform_role}
                      disabled={roleSelectDisabled}
                      onChange={(e) => handleRoleChange(u.id, e.target.value as PlatformRole)}
                      className="rounded-md border border-border bg-background px-2 py-1 text-sm disabled:opacity-50"
                    >
                      {ROLES.map((role) => (
                        <option
                          key={role}
                          value={role}
                          disabled={
                            (role === "amministratore" || role === "super_admin") && !canGrantPrivilegedRoles
                          }
                        >
                          {role}
                        </option>
                      ))}
                    </select>
                  </td>
                  <td className="px-4 py-3">
                    <Button
                      variant={u.is_active ? "secondary" : "primary"}
                      disabled={u.id === me?.id && u.is_active}
                      onClick={() => handleToggleActive(u.id, !u.is_active)}
                    >
                      {u.is_active ? "Disattiva" : "Attiva"}
                    </Button>
                    {rowError[u.id] && <p className="mt-1 text-xs text-red-700">{rowError[u.id]}</p>}
                  </td>
                </tr>
              );
            })}
            {users !== null && users.length === 0 && (
              <tr>
                <td colSpan={4} className="px-4 py-6 text-center text-muted">
                  Nessun utente trovato.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
