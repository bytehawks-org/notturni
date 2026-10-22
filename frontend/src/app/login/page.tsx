"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { Suspense, useEffect, useState, type FormEvent } from "react";

import { OtpInput, OTP_INPUT_LENGTH } from "@/components/auth/OtpInput";
import { SsoButtons } from "@/components/auth/SsoButtons";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { isMfaRequired } from "@/lib/types";

const OTP_LENGTH = OTP_INPUT_LENGTH;

export default function LoginPage() {
  return (
    <Suspense>
      <LoginForm />
    </Suspense>
  );
}

function LoginForm() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const { login, verifyMfa } = useAuth();
  const t = useTranslations("Auth");
  const tc = useTranslations("Common");

  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [code, setCode] = useState("");
  const [ssoProviders, setSsoProviders] = useState<string[]>([]);
  const [submitting, setSubmitting] = useState(false);

  // Arrivo da un redirect del backend dopo un login SSO (navigazione vera
  // del browser, non una fetch — backend/app/api/v1/auth.py::sso_callback
  // non può restituire il challenge nel corpo di una risposta letta da JS,
  // lo passa in query string): stato iniziale letto direttamente da
  // useSearchParams(), disponibile già al primo render, non da un effect
  // che chiamerebbe setState sincronicamente. `mfa_challenge`/`mfa_method`
  // arrivano quando l'account ha già l'MFA attivo (collegamento del
  // provider in sospeso); `sso_error` per un provider non configurato,
  // negato dall'utente o senza email disponibile.
  const [challenge, setChallenge] = useState<string | null>(() => searchParams.get("mfa_challenge"));
  const [mfaMethod, setMfaMethod] = useState<string | null>(() => searchParams.get("mfa_method"));
  const [error, setError] = useState<string | null>(() => (searchParams.get("sso_error") ? t("ssoError") : null));

  useEffect(() => {
    api.config
      .get()
      .then((c) => setSsoProviders(c.sso_providers ?? []))
      .catch(() => undefined);
  }, []);

  // Ripulisce la query string letta sopra, non lasciarla visibile/
  // ricaricabile dalla history del browser.
  useEffect(() => {
    if (searchParams.get("mfa_challenge") || searchParams.get("sso_error")) {
      router.replace("/login");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function handleLogin(event: FormEvent) {
    event.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      const res = await login(email, password);
      if (isMfaRequired(res)) {
        setChallenge(res.challenge);
        setMfaMethod(res.method);
      } else {
        router.push("/dashboard");
      }
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : tc("unexpectedError"));
    } finally {
      setSubmitting(false);
    }
  }

  async function handleVerify(event: FormEvent) {
    event.preventDefault();
    if (!challenge) return;
    setError(null);
    setSubmitting(true);
    try {
      await verifyMfa(challenge, code);
      router.push("/dashboard");
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : tc("unexpectedError"));
    } finally {
      setSubmitting(false);
    }
  }

  function handleBack() {
    setChallenge(null);
    setMfaMethod(null);
    setCode("");
    setError(null);
  }

  return (
    <main className="flex flex-1 items-center justify-center px-6 py-16">
      <div className="w-full max-w-sm">
        <Link href="/" className="font-serif text-2xl font-semibold text-foreground no-underline">
          Notturni
        </Link>

        {!challenge ? (
          <form onSubmit={handleLogin} className="mt-7 flex flex-col gap-6">
            <div className="flex flex-col gap-1.5">
              <h1 className="font-serif text-[30px] font-medium leading-[1.15] text-foreground">{t("welcomeBack")}</h1>
              <p className="text-[15px] text-muted">{t("loginIntro")}</p>
            </div>

            <div className="flex flex-col gap-3.5">
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
              <FieldGroup className="mb-0">
                <Label htmlFor="password">{t("password")}</Label>
                <Input
                  id="password"
                  type="password"
                  required
                  autoComplete="current-password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                />
                <Link href="/forgot-password" className="mt-1 self-end text-[13px] text-muted no-underline hover:text-primary hover:underline">
                  {t("forgotPassword")}
                </Link>
              </FieldGroup>
            </div>

            {error && <Alert kind="error">{error}</Alert>}

            <Button type="submit" size="lg" disabled={submitting}>
              {submitting ? t("signingIn") : t("continue")}
            </Button>

            <SsoButtons providers={ssoProviders} />

            <p className="text-center text-[13px] leading-relaxed text-muted">
              {t("noAccount")}{" "}
              <Link href="/register" className="text-primary no-underline hover:underline">
                {t("createOne")}
              </Link>
            </p>
          </form>
        ) : (
          <form onSubmit={handleVerify} className="mt-7 flex flex-col gap-6">
            <button
              type="button"
              onClick={handleBack}
              className="self-start text-[15px] text-muted hover:text-foreground"
            >
              {tc("back")}
            </button>

            <div className="flex flex-col gap-1.5">
              <span className="font-mono text-xs font-semibold uppercase tracking-[.06em] text-primary">
                {t("step2")}
              </span>
              <h1 className="font-serif text-[30px] font-medium leading-[1.15] text-foreground">
                {t("confirmYou")}
              </h1>
              <p className="text-[15px] leading-relaxed text-muted">
                {mfaMethod === "email" ? t("otpEmail") : t("otpApp")}
              </p>
            </div>

            <OtpInput
              value={code}
              onChange={setCode}
              legend={t("otpLegend")}
              digitLabel={(index, total) => t("otpDigitLabel", { index, total })}
            />

            {error && <Alert kind="error">{error}</Alert>}

            <Button type="submit" size="lg" disabled={submitting || code.length !== OTP_LENGTH}>
              {submitting ? t("verifying") : t("verify")}
            </Button>
          </form>
        )}
      </div>
    </main>
  );
}
