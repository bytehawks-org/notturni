"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useState, type FormEvent } from "react";

import { OtpInput, OTP_INPUT_LENGTH } from "@/components/auth/OtpInput";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { api, ApiClientError } from "@/lib/api";

/** /forgot-password (CLAUDE.md #3, hardening alpha): due passi come il login
 * con MFA — email, poi codice + nuova password. Il passo 1 mostra sempre lo
 * stesso messaggio di conferma (`POST /auth/password/forgot` risponde sempre
 * 202): l'esistenza di un account non deve essere enumerabile provando email
 * a caso. */
export default function ForgotPasswordPage() {
  const router = useRouter();
  const t = useTranslations("Auth");
  const tc = useTranslations("Common");

  const [email, setEmail] = useState("");
  const [sent, setSent] = useState(false);
  const [code, setCode] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [done, setDone] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleRequest(event: FormEvent) {
    event.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      await api.auth.forgotPassword({ email });
      setSent(true);
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : tc("unexpectedError"));
    } finally {
      setSubmitting(false);
    }
  }

  async function handleReset(event: FormEvent) {
    event.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      await api.auth.resetPassword({ email, code, new_password: newPassword });
      setDone(true);
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : tc("unexpectedError"));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <main className="flex flex-1 items-center justify-center px-6 py-16">
      <div className="w-full max-w-sm">
        <Link href="/" className="font-serif text-2xl font-semibold text-foreground no-underline">
          Notturni
        </Link>

        {done ? (
          <div className="mt-7 flex flex-col gap-6">
            <h1 className="font-serif text-[30px] font-medium leading-[1.15] text-foreground">{t("resetPassword")}</h1>
            <Alert kind="success">{t("resetSuccess")}</Alert>
            <Button size="lg" onClick={() => router.push("/login")}>
              {t("backToLogin")}
            </Button>
          </div>
        ) : !sent ? (
          <form onSubmit={handleRequest} className="mt-7 flex flex-col gap-6">
            <div className="flex flex-col gap-1.5">
              <h1 className="font-serif text-[30px] font-medium leading-[1.15] text-foreground">
                {t("forgotPasswordTitle")}
              </h1>
              <p className="text-[15px] text-muted">{t("forgotPasswordIntro")}</p>
            </div>

            <FieldGroup className="mb-0">
              <Label htmlFor="email">{t("email")}</Label>
              <Input
                id="email"
                type="email"
                required
                autoComplete="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
              />
            </FieldGroup>

            {error && <Alert kind="error">{error}</Alert>}

            <Button type="submit" size="lg" disabled={submitting}>
              {submitting ? t("sendingCode") : t("sendCode")}
            </Button>

            <p className="text-center text-[13px] leading-relaxed text-muted">
              <Link href="/login" className="text-primary no-underline hover:underline">
                {t("backToLogin")}
              </Link>
            </p>
          </form>
        ) : (
          <form onSubmit={handleReset} className="mt-7 flex flex-col gap-6">
            <div className="flex flex-col gap-1.5">
              <h1 className="font-serif text-[30px] font-medium leading-[1.15] text-foreground">
                {t("forgotPasswordTitle")}
              </h1>
              <p className="text-[15px] leading-relaxed text-muted">{t("checkYourEmail")}</p>
              <p className="text-[15px] text-muted">{t("resetPasswordIntro")}</p>
            </div>

            <OtpInput
              value={code}
              onChange={setCode}
              legend={t("otpLegend")}
              digitLabel={(index, total) => t("otpDigitLabel", { index, total })}
            />

            <FieldGroup className="mb-0">
              <Label htmlFor="new-password" hint={t("passwordHint")}>
                {t("newPassword")}
              </Label>
              <Input
                id="new-password"
                type="password"
                required
                minLength={10}
                autoComplete="new-password"
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
              />
            </FieldGroup>

            {error && <Alert kind="error">{error}</Alert>}

            <Button type="submit" size="lg" disabled={submitting || code.length !== OTP_INPUT_LENGTH}>
              {submitting ? t("resetting") : t("resetPassword")}
            </Button>
          </form>
        )}
      </div>
    </main>
  );
}
