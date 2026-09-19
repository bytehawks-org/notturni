"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";

const USERNAME_CHECK_DEBOUNCE_MS = 300;

type UsernameStatus =
  | { state: "idle" }
  | { state: "checking" }
  | { state: "available" }
  | { state: "unavailable"; reason: "invalid_format" | "taken" | null };

interface UsernameCheckResult {
  username: string;
  available: boolean;
  reason: "invalid_format" | "taken" | null;
}

export default function RegisterPage() {
  const router = useRouter();
  const { register, login } = useAuth();
  const t = useTranslations("Auth");
  const tc = useTranslations("Common");

  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [repeatPassword, setRepeatPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [registrationMode, setRegistrationMode] = useState<"open" | "invite" | "closed" | null>(null);
  const [usernameCheck, setUsernameCheck] = useState<UsernameCheckResult | null>(null);

  useEffect(() => {
    api.config
      .get()
      .then((c) => setRegistrationMode(c.registration_mode ?? "open"))
      .catch(() => setRegistrationMode("open"));
  }, []);

  useEffect(() => {
    if (!username || username.length < 3) return;
    const timeout = window.setTimeout(() => {
      api.auth
        .usernameAvailable(username)
        .then((res) => setUsernameCheck({ username, available: res.available, reason: res.reason }))
        .catch(() => undefined);
    }, USERNAME_CHECK_DEBOUNCE_MS);
    return () => window.clearTimeout(timeout);
  }, [username]);

  // Derivato dal render, non da uno stato aggiornato nell'effetto: evita un
  // setState sincrono nel corpo dell'effetto (react-hooks/set-state-in-effect)
  // e tiene "checking" coerente da solo finché la verifica per lo username
  // corrente non è ancora tornata (o lo username è cambiato nel frattempo).
  const usernameStatus: UsernameStatus =
    !username || username.length < 3
      ? { state: "idle" }
      : usernameCheck?.username !== username
        ? { state: "checking" }
        : usernameCheck.available
          ? { state: "available" }
          : { state: "unavailable", reason: usernameCheck.reason };

  const passwordsMismatch = repeatPassword.length > 0 && password !== repeatPassword;
  const canSubmit =
    usernameStatus.state !== "checking" && usernameStatus.state !== "unavailable" && !passwordsMismatch;

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    setError(null);
    if (password !== repeatPassword) {
      setError(t("repeatPasswordMismatch"));
      return;
    }
    setSubmitting(true);
    try {
      await register(username, email, password);
      // niente login automatico lato backend: lo facciamo qui per comodità
      await login(email, password);
      router.push("/dashboard");
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

        {registrationMode && registrationMode !== "open" && (
          <div className="mt-7 flex flex-col gap-3">
            <h1 className="font-serif text-[30px] font-medium leading-[1.15] text-foreground">{t("createAccount")}</h1>
            <Alert kind="info">{registrationMode === "closed" ? t("registrationClosed") : t("registrationInvite")}</Alert>
            <p className="text-[13px] text-muted">
              {t("haveAccount")}{" "}
              <Link href="/login" className="text-primary no-underline hover:underline">
                {t("signIn")}
              </Link>
            </p>
          </div>
        )}
        {(!registrationMode || registrationMode === "open") && (
        <form onSubmit={handleSubmit} className="mt-7 flex flex-col gap-6">
          <div className="flex flex-col gap-1.5">
            <h1 className="font-serif text-[30px] font-medium leading-[1.15] text-foreground">{t("createAccount")}</h1>
            <p className="text-[15px] text-muted">{t("registerIntro")}</p>
          </div>

          <div className="flex flex-col gap-3.5">
            <FieldGroup className="mb-0">
              <Label htmlFor="username">{t("username")}</Label>
              <Input
                id="username"
                required
                minLength={3}
                value={username}
                onChange={(e) => setUsername(e.target.value)}
              />
              {usernameStatus.state === "checking" && (
                <p className="mt-1 text-xs text-muted">{t("usernameChecking")}</p>
              )}
              {usernameStatus.state === "available" && (
                <p className="mt-1 text-xs text-primary">{t("usernameAvailable")}</p>
              )}
              {usernameStatus.state === "unavailable" && (
                <p className="mt-1 text-xs text-danger">
                  {usernameStatus.reason === "invalid_format" ? t("usernameInvalidFormat") : t("usernameTaken")}
                </p>
              )}
            </FieldGroup>
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
              <Label htmlFor="password" hint={t("passwordHint")}>
                {t("password")}
              </Label>
              <Input
                id="password"
                type="password"
                required
                minLength={10}
                autoComplete="new-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </FieldGroup>
            <FieldGroup className="mb-0">
              <Label htmlFor="repeat-password">{t("repeatPassword")}</Label>
              <Input
                id="repeat-password"
                type="password"
                required
                minLength={10}
                autoComplete="new-password"
                value={repeatPassword}
                onChange={(e) => setRepeatPassword(e.target.value)}
              />
              {passwordsMismatch && <p className="mt-1 text-xs text-danger">{t("repeatPasswordMismatch")}</p>}
            </FieldGroup>
          </div>

          {error && <Alert kind="error">{error}</Alert>}

          <Button type="submit" size="lg" disabled={submitting || !canSubmit}>
            {submitting ? t("creating") : t("createAccount")}
          </Button>

          <p className="text-center text-[13px] leading-relaxed text-muted">
            {t("haveAccount")}{" "}
            <Link href="/login" className="text-primary no-underline hover:underline">
              {t("signIn")}
            </Link>
          </p>
        </form>
        )}
      </div>
    </main>
  );
}
