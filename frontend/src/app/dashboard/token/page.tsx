"use client";

import { useLocale, useTranslations } from "next-intl";
import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDateTime } from "@/lib/format";
import type { ApiToken } from "@/lib/types";

export default function DashboardTokenPage() {
  const { authFetch } = useAuth();
  const t = useTranslations("Tokens");
  const tc = useTranslations("Common");
  const locale = useLocale();
  const [tokens, setTokens] = useState<ApiToken[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [name, setName] = useState("");
  const [creating, setCreating] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const [justCreated, setJustCreated] = useState<string | null>(null);

  const errorMessage = useCallback(
    (err: unknown) => (err instanceof ApiClientError ? err.message : tc("unexpectedError")),
    [tc]
  );

  const load = useCallback(() => {
    authFetch((token) => api.tokens.list(token))
      .then((list) => {
        setTokens(list);
        setError(null);
      })
      .catch((err) => setError(errorMessage(err)));
  }, [authFetch, errorMessage]);

  useEffect(load, [load]);

  async function handleCreate(e: FormEvent) {
    e.preventDefault();
    setCreating(true);
    setCreateError(null);
    try {
      const created = await authFetch((token) => api.tokens.create(token, name));
      setJustCreated(created.token);
      setName("");
      load();
    } catch (err) {
      setCreateError(errorMessage(err));
    } finally {
      setCreating(false);
    }
  }

  async function handleRevoke(tokenId: string) {
    if (!window.confirm(t("confirmRevoke"))) return;
    try {
      await authFetch((token) => api.tokens.revoke(token, tokenId));
      load();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  const active = tokens?.filter((tk) => !tk.revoked_at) ?? null;

  return (
    <div className="mx-auto flex max-w-4xl flex-col gap-8">
      <div className="flex flex-col gap-1">
        <h1 className="font-serif text-[28px] font-medium leading-tight text-foreground">{t("title")}</h1>
        <p className="text-sm text-muted">{t("intro")}</p>
      </div>

      <Card className="flex flex-col gap-3">
        <CardTitle>{t("newToken")}</CardTitle>
        <form onSubmit={handleCreate} className="flex items-end gap-3">
          <div className="flex-1">
            <FieldGroup className="mb-0">
              <Label htmlFor="token-name">{t("name")}</Label>
              <Input id="token-name" required placeholder={t("namePlaceholder")} value={name} onChange={(e) => setName(e.target.value)} />
            </FieldGroup>
          </div>
          <Button type="submit" disabled={creating}>
            {t("create")}
          </Button>
        </form>
        {createError && <Alert kind="error">{createError}</Alert>}
        {justCreated && (
          <div className="flex flex-col gap-2">
            <p className="break-all rounded-md border border-border bg-foreground/5 p-3 font-mono text-xs">{justCreated}</p>
            <Alert kind="info">{t("shownOnce")}</Alert>
          </div>
        )}
      </Card>

      <section className="flex flex-col gap-3">
        <CardTitle>{t("active")}</CardTitle>
        {error && <Alert kind="error">{error}</Alert>}
        {active === null && !error && <SkeletonRows rows={3} />}
        {active !== null && active.length === 0 && <EmptyState glyph="◈" title={t("emptyTitle")} body={t("emptyBody")} />}
        {active !== null && active.length > 0 && (
          <div className="overflow-x-auto rounded-xl border border-border bg-surface">
            <table className="w-full text-left text-sm">
              <thead className="border-b border-border font-mono text-[11px] uppercase tracking-[.06em] text-muted">
                <tr>
                  <th className="px-4 py-3">{t("col.name")}</th>
                  <th className="px-4 py-3">{t("col.prefix")}</th>
                  <th className="whitespace-nowrap px-4 py-3">{t("col.created")}</th>
                  <th className="whitespace-nowrap px-4 py-3">{t("col.lastUsed")}</th>
                  <th className="px-4 py-3" />
                </tr>
              </thead>
              <tbody>
                {active.map((tk) => (
                  <tr key={tk.id} className="border-b border-border align-top last:border-0">
                    <td className="px-4 py-3 text-foreground">{tk.name}</td>
                    <td className="px-4 py-3 font-mono text-xs text-muted">{tk.token_prefix}…</td>
                    <td className="whitespace-nowrap px-4 py-3 text-muted">{formatDateTime(tk.created_at, locale)}</td>
                    <td className="whitespace-nowrap px-4 py-3 text-muted">{tk.last_used_at ? formatDateTime(tk.last_used_at, locale) : t("never")}</td>
                    <td className="px-4 py-3 text-right">
                      <Button variant="danger" size="sm" onClick={() => handleRevoke(tk.id)}>
                        {t("revoke")}
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}
