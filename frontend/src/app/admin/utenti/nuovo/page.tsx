"use client";

import Link from "next/link";
import { useTranslations } from "next-intl";
import { useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { Input, Label } from "@/components/ui/Field";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { CreatedUser } from "@/lib/types";

/** Super Admin form to create a collaborator account. The generated password lives only in component state:
 * it is shown once, never logged and never written to any storage. */
export default function NewUserPage() {
  const { user, authFetch } = useAuth();
  const t = useTranslations("AdminUserCreate");
  const tc = useTranslations("Common");
  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [created, setCreated] = useState<CreatedUser | null>(null);
  const [copied, setCopied] = useState(false);

  if (user && user.platform_role !== "super_admin") return <Alert kind="error">{t("superAdminOnly")}</Alert>;

  function describe(err: unknown): string {
    if (err instanceof ApiClientError) {
      if (err.status === 409) return t("duplicate");
      if (err.status === 400) return t("invalid", { message: err.message });
      return err.message;
    }
    return tc("unexpectedError");
  }

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    const payload: { username: string; email: string; password?: string } = { username: username.trim(), email: email.trim() };
    if (password) payload.password = password;
    try {
      const res = await authFetch((token) => api.admin.createUser(token, payload));
      setCreated(res);
      setPassword("");
    } catch (err) {
      setError(describe(err));
    } finally {
      setBusy(false);
    }
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  }

  function reset() {
    setCreated(null);
    setUsername("");
    setEmail("");
    setCopied(false);
  }

  return (
    <div className="mx-auto flex max-w-2xl flex-col gap-6">
      <header className="flex flex-col gap-1">
        <h1 className="font-serif text-[28px] font-medium leading-tight text-foreground">{t("title")}</h1>
        <p className="text-sm text-muted">{t("subtitle")}</p>
      </header>

      {created ? (
        <Card className="flex flex-col gap-4">
          <CardTitle>{t("createdTitle", { username: created.username })}</CardTitle>
          {created.generated_password ? (
            <>
              <Alert kind="error">{t("passwordWarning")}</Alert>
              <div className="flex items-center gap-2">
                <code data-testid="generated-password" className="flex-1 break-all rounded-md border border-border bg-background px-3 py-2 font-mono text-sm">
                  {created.generated_password}
                </code>
                <Button size="sm" variant="secondary" onClick={() => copy(created.generated_password ?? "")}>
                  {copied ? t("copied") : t("copy")}
                </Button>
              </div>
            </>
          ) : (
            <p className="text-sm text-muted">{t("customPasswordSet")}</p>
          )}
          <p className="text-[13px] text-muted">{t("inviteHelp")}</p>
          <div className="flex gap-2">
            <Button onClick={reset}>{t("createAnother")}</Button>
            <Link href="/admin/utenti" className="inline-flex items-center text-sm text-primary no-underline hover:underline">
              {t("backToUsers")}
            </Link>
          </div>
        </Card>
      ) : (
        <Card>
          <form onSubmit={submit} className="flex flex-col gap-4">
            {error && <Alert kind="error">{error}</Alert>}
            <div>
              <Label htmlFor="new-username">{t("username")}</Label>
              <Input id="new-username" value={username} onChange={(e) => setUsername(e.target.value)} required autoComplete="off" />
            </div>
            <div>
              <Label htmlFor="new-email">{t("email")}</Label>
              <Input id="new-email" type="email" value={email} onChange={(e) => setEmail(e.target.value)} required autoComplete="off" />
            </div>
            <div>
              <Label htmlFor="new-password" hint={t("passwordHint")}>
                {t("password")}
              </Label>
              <Input id="new-password" type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="new-password" />
            </div>
            <p className="text-[13px] text-muted">{t("inviteHelp")}</p>
            <div>
              <Button type="submit" disabled={busy || !username.trim() || !email.trim()}>
                {busy ? tc("saving") : t("submit")}
              </Button>
            </div>
          </form>
        </Card>
      )}
    </div>
  );
}
