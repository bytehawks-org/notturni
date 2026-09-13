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

export default function RegisterPage() {
  const router = useRouter();
  const { register, login } = useAuth();
  const t = useTranslations("Auth");
  const tc = useTranslations("Common");

  const [username, setUsername] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [registrationMode, setRegistrationMode] = useState<"open" | "invite" | "closed" | null>(null);

  useEffect(() => {
    api.config
      .get()
      .then((c) => setRegistrationMode(c.registration_mode ?? "open"))
      .catch(() => setRegistrationMode("open"));
  }, []);

  async function handleSubmit(event: FormEvent) {
    event.preventDefault();
    setError(null);
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
                minLength={8}
                autoComplete="new-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </FieldGroup>
          </div>

          {error && <Alert kind="error">{error}</Alert>}

          <Button type="submit" size="lg" disabled={submitting}>
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
