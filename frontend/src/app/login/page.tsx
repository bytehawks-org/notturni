"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useRef, useState, type FormEvent, type KeyboardEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { ApiClientError } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { isMfaRequired } from "@/lib/types";

const OTP_LENGTH = 6;

/** Sei caselle per cifra (mockup 1h) invece di un unico campo testuale. */
function OtpInput({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  const refs = useRef<(HTMLInputElement | null)[]>([]);
  const digits = Array.from({ length: OTP_LENGTH }, (_, i) => value[i] ?? "");

  function setDigit(index: number, digit: string) {
    const next = digits.slice();
    next[index] = digit;
    onChange(next.join(""));
    if (digit && index < OTP_LENGTH - 1) refs.current[index + 1]?.focus();
  }

  function handleKeyDown(index: number, event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Backspace" && !digits[index] && index > 0) {
      refs.current[index - 1]?.focus();
    }
  }

  function handlePaste(event: React.ClipboardEvent<HTMLInputElement>) {
    const pasted = event.clipboardData.getData("text").replace(/\D/g, "").slice(0, OTP_LENGTH);
    if (!pasted) return;
    event.preventDefault();
    onChange(pasted.padEnd(value.length, ""));
    refs.current[Math.min(pasted.length, OTP_LENGTH - 1)]?.focus();
  }

  return (
    <div className="grid grid-cols-6 gap-2">
      {digits.map((digit, i) => (
        <input
          key={i}
          ref={(el) => {
            refs.current[i] = el;
          }}
          type="text"
          inputMode="numeric"
          maxLength={1}
          autoFocus={i === 0}
          value={digit}
          onChange={(e) => setDigit(i, e.target.value.replace(/\D/g, "").slice(-1))}
          onKeyDown={(e) => handleKeyDown(i, e)}
          onPaste={handlePaste}
          className="h-[52px] rounded-xl border border-border bg-surface text-center font-serif text-2xl text-foreground focus:outline-none focus:border-primary focus:ring-[3px] focus:ring-primary/20"
        />
      ))}
    </div>
  );
}

export default function LoginPage() {
  const router = useRouter();
  const { login, verifyMfa } = useAuth();
  const t = useTranslations("Auth");
  const tc = useTranslations("Common");

  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const [challenge, setChallenge] = useState<string | null>(null);
  const [mfaMethod, setMfaMethod] = useState<string | null>(null);
  const [code, setCode] = useState("");

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
              </FieldGroup>
            </div>

            {error && <Alert kind="error">{error}</Alert>}

            <Button type="submit" size="lg" disabled={submitting}>
              {submitting ? t("signingIn") : t("continue")}
            </Button>

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

            <OtpInput value={code} onChange={setCode} />

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
