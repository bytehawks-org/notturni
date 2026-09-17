"use client";

import { useTranslations } from "next-intl";
import Image from "next/image";
import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { Input, Label } from "@/components/ui/Field";
import { Pill } from "@/components/ui/Pill";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import {
  INVITABLE_BLOG_ROLES,
  type BlogInvitation,
  type BlogMember,
  type BlogRole,
} from "@/lib/types";

import { errorMessage, roleMessageKey } from "./shared";

const hue = (s: string) => `oklch(0.55 0.06 ${[...s].reduce((a, c) => a + c.charCodeAt(0), 0) % 360})`;

export function CollaboratorsTab({ blogSlug }: { blogSlug: string }) {
  const { authFetch } = useAuth();
  const t = useTranslations("CollaboratorsTab");
  const ta = useTranslations("BlogAdmin");
  const tc = useTranslations("Common");
  const [members, setMembers] = useState<BlogMember[] | null>(null);
  const [invitations, setInvitations] = useState<BlogInvitation[]>([]);
  const [username, setUsername] = useState("");
  const [role, setRole] = useState<BlogRole>(INVITABLE_BLOG_ROLES[0]);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    authFetch((token) => api.blogs.members(token, blogSlug))
      .then(setMembers)
      .catch((err) => setError(errorMessage(err, tc("unexpectedError"))));
    authFetch((token) => api.blogs.listInvitations(token, blogSlug))
      .then(setInvitations)
      .catch(() => undefined);
  }, [authFetch, blogSlug, tc]);

  useEffect(load, [load]);

  async function handleInvite(event: FormEvent) {
    event.preventDefault();
    setError(null);
    try {
      await authFetch((token) => api.blogs.createInvitation(token, blogSlug, username, role));
      setUsername("");
      load();
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    }
  }

  async function handleRevoke(invitationId: string) {
    try {
      await authFetch((token) => api.blogs.revokeInvitation(token, blogSlug, invitationId));
      load();
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    }
  }

  async function handleRemoveMember(userId: string) {
    try {
      await authFetch((token) => api.blogs.removeMember(token, blogSlug, userId));
      load();
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    }
  }

  async function handleChangeRole(userId: string, newRole: BlogRole) {
    try {
      await authFetch((token) => api.blogs.updateMemberRole(token, blogSlug, userId, newRole));
      load();
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    }
  }

  const pending = invitations.filter((i) => i.status === "pending");

  return (
    <div className="space-y-6">
      {error && <Alert kind="error">{error}</Alert>}

      <Card className="flex flex-col gap-3">
        <CardTitle>{t("title")}</CardTitle>
        {members !== null && members.length === 0 && (
          <p className="text-sm text-muted">{t("empty")}</p>
        )}
        {members && members.length > 0 && (
          <div className="overflow-hidden rounded-lg border border-border">
            {members.map((m) => (
              <div
                key={m.user_id}
                className="flex flex-wrap items-center justify-between gap-3 border-b border-border px-4 py-2.5 text-sm last:border-0"
              >
                <div className="flex min-w-0 items-center gap-2.5">
                  {m.avatar_url ? (
                    <Image
                      src={m.avatar_url}
                      alt={m.username}
                      width={28}
                      height={28}
                      className="h-7 w-7 flex-none rounded-full object-cover"
                      unoptimized
                    />
                  ) : (
                    <span
                      className="grid h-7 w-7 flex-none place-items-center rounded-full text-xs font-semibold text-white"
                      style={{ background: hue(m.username) }}
                      aria-hidden="true"
                    >
                      {m.username[0]?.toUpperCase()}
                    </span>
                  )}
                  <div className="flex min-w-0 flex-col leading-tight">
                    <span className="text-foreground">@{m.username}</span>
                    {m.author_display_name && (
                      <span className="truncate text-xs text-muted">{t("signsAs", { alias: m.author_display_name })}</span>
                    )}
                  </div>
                </div>
                <span className="flex items-center gap-2.5">
                  <Pill tone="primary">{ta(`roles.${roleMessageKey(m.role)}`)}</Pill>
                  <select
                    value={m.role}
                    onChange={(e) => handleChangeRole(m.user_id, e.target.value as BlogRole)}
                    className="rounded-lg border border-border bg-surface px-2 py-1.5 text-sm text-foreground focus:outline-none focus:border-primary focus:ring-[3px] focus:ring-primary/20"
                  >
                    {INVITABLE_BLOG_ROLES.map((r) => (
                      <option key={r} value={r}>
                        {ta(`roles.${roleMessageKey(r)}`)}
                      </option>
                    ))}
                  </select>
                  <button
                    type="button"
                    onClick={() => handleRemoveMember(m.user_id)}
                    className="text-muted hover:text-foreground"
                  >
                    {t("remove")}
                  </button>
                </span>
              </div>
            ))}
          </div>
        )}
      </Card>

      <Card className="flex flex-col gap-3">
        <div className="flex items-baseline justify-between">
          <CardTitle>{t("inviteTitle")}</CardTitle>
          <span className="text-[13px] text-muted">{t("inviteHint")}</span>
        </div>
        <form onSubmit={handleInvite} className="flex flex-wrap items-end gap-3">
          <div>
            <Label htmlFor="invite-username">{t("username")}</Label>
            <Input
              id="invite-username"
              required
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
          </div>
          <div>
            <Label htmlFor="invite-role">{t("role")}</Label>
            <select
              id="invite-role"
              value={role}
              onChange={(e) => setRole(e.target.value as BlogRole)}
              className="rounded-lg border border-border bg-surface px-3 py-2.5 text-sm text-foreground focus:outline-none focus:border-primary focus:ring-[3px] focus:ring-primary/20"
            >
              {INVITABLE_BLOG_ROLES.map((r) => (
                <option key={r} value={r}>
                  {ta(`roles.${roleMessageKey(r)}`)}
                </option>
              ))}
            </select>
          </div>
          <Button type="submit">{t("submit")}</Button>
        </form>

        {pending.length > 0 && (
          <div className="overflow-hidden rounded-lg border border-border">
            {pending.map((inv) => (
              <div key={inv.id} className="flex items-center justify-between border-b border-border px-4 py-2.5 text-sm last:border-0">
                <span className="text-foreground">
                  @{inv.invited_username} — {ta(`roles.${roleMessageKey(inv.role)}`)}{" "}
                  <span className="text-muted">{t("pending")}</span>
                </span>
                <button
                  type="button"
                  onClick={() => handleRevoke(inv.id)}
                  className="text-muted hover:text-foreground"
                >
                  {t("revoke")}
                </button>
              </div>
            ))}
          </div>
        )}
      </Card>
    </div>
  );
}
