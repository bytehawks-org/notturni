"use client";

import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { Input, Label } from "@/components/ui/Field";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import {
  INVITABLE_BLOG_ROLES,
  type BlogInvitation,
  type BlogMember,
  type BlogRole,
} from "@/lib/types";

import { errorMessage, ROLE_LABELS } from "./shared";

export function CollaboratorsTab({ blogSlug }: { blogSlug: string }) {
  const { authFetch } = useAuth();
  const [members, setMembers] = useState<BlogMember[] | null>(null);
  const [invitations, setInvitations] = useState<BlogInvitation[]>([]);
  const [username, setUsername] = useState("");
  const [role, setRole] = useState<BlogRole>(INVITABLE_BLOG_ROLES[0].value);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    authFetch((token) => api.blogs.members(token, blogSlug))
      .then(setMembers)
      .catch((err) => setError(errorMessage(err)));
    authFetch((token) => api.blogs.listInvitations(token, blogSlug))
      .then(setInvitations)
      .catch(() => undefined);
  }, [authFetch, blogSlug]);

  useEffect(load, [load]);

  async function handleInvite(event: FormEvent) {
    event.preventDefault();
    setError(null);
    try {
      await authFetch((token) => api.blogs.createInvitation(token, blogSlug, username, role));
      setUsername("");
      load();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  async function handleRevoke(invitationId: string) {
    try {
      await authFetch((token) => api.blogs.revokeInvitation(token, blogSlug, invitationId));
      load();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  async function handleRemoveMember(userId: string) {
    try {
      await authFetch((token) => api.blogs.removeMember(token, blogSlug, userId));
      load();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  async function handleChangeRole(userId: string, newRole: BlogRole) {
    try {
      await authFetch((token) => api.blogs.updateMemberRole(token, blogSlug, userId, newRole));
      load();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  const pending = invitations.filter((i) => i.status === "pending");

  return (
    <div className="space-y-6">
      {error && <Alert kind="error">{error}</Alert>}

      <Card>
        <CardTitle>Collaboratori</CardTitle>
        {members !== null && members.length === 0 && (
          <p className="text-sm text-muted">Nessun collaboratore.</p>
        )}
        <ul className="space-y-2">
          {members?.map((m) => (
            <li key={m.user_id} className="flex flex-wrap items-center justify-between gap-3 text-sm">
              <span className="text-foreground">
                @{m.username}
                {m.author_display_name && (
                  <span className="text-muted"> — firma come «{m.author_display_name}»</span>
                )}
              </span>
              <span className="flex items-center gap-2">
                <select
                  value={m.role}
                  onChange={(e) => handleChangeRole(m.user_id, e.target.value as BlogRole)}
                  className="rounded-md border border-border bg-background px-2 py-1 text-sm text-foreground"
                >
                  {INVITABLE_BLOG_ROLES.map((r) => (
                    <option key={r.value} value={r.value}>
                      {r.label}
                    </option>
                  ))}
                </select>
                <button
                  type="button"
                  onClick={() => handleRemoveMember(m.user_id)}
                  className="text-muted hover:text-foreground"
                >
                  Rimuovi
                </button>
              </span>
            </li>
          ))}
        </ul>
      </Card>

      <Card>
        <CardTitle>Invita un collaboratore</CardTitle>
        <p className="mb-4 text-sm text-muted">
          L&apos;invito resta in attesa finché l&apos;utente non lo accetta dalla propria dashboard.
        </p>
        <form onSubmit={handleInvite} className="flex flex-wrap items-end gap-3">
          <div>
            <Label htmlFor="invite-username">Username</Label>
            <Input
              id="invite-username"
              required
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
          </div>
          <div>
            <Label htmlFor="invite-role">Ruolo</Label>
            <select
              id="invite-role"
              value={role}
              onChange={(e) => setRole(e.target.value as BlogRole)}
              className="rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground"
            >
              {INVITABLE_BLOG_ROLES.map((r) => (
                <option key={r.value} value={r.value}>
                  {r.label}
                </option>
              ))}
            </select>
          </div>
          <Button type="submit">Invia invito</Button>
        </form>

        {pending.length > 0 && (
          <ul className="mt-4 space-y-2">
            {pending.map((inv) => (
              <li key={inv.id} className="flex items-center justify-between text-sm">
                <span className="text-foreground">
                  @{inv.invited_username} — {ROLE_LABELS[inv.role] ?? inv.role} (in attesa)
                </span>
                <button
                  type="button"
                  onClick={() => handleRevoke(inv.id)}
                  className="text-muted hover:text-foreground"
                >
                  Revoca
                </button>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}
