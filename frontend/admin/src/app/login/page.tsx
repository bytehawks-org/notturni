"use client";

import { useRouter } from "next/navigation";
import { useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { ApiClientError } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { isMfaRequired } from "@/lib/types";

export default function LoginPage() {
  const router = useRouter();
  const { login, verifyMfa } = useAuth();

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
        router.push("/");
      }
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : "Errore imprevisto.");
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
      router.push("/");
    } catch (err) {
      setError(err instanceof ApiClientError ? err.message : "Errore imprevisto.");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <main className="flex flex-1 items-center justify-center px-6 py-16">
      <Card className="w-full max-w-sm">
        <CardTitle>Amministrazione</CardTitle>

        {!challenge ? (
          <form onSubmit={handleLogin}>
            <FieldGroup>
              <Label htmlFor="email">Email</Label>
              <Input
                id="email"
                type="email"
                required
                autoComplete="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
              />
            </FieldGroup>
            <FieldGroup>
              <Label htmlFor="password">Password</Label>
              <Input
                id="password"
                type="password"
                required
                autoComplete="current-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
            </FieldGroup>
            {error && (
              <div className="mb-4">
                <Alert kind="error">{error}</Alert>
              </div>
            )}
            <Button type="submit" disabled={submitting} className="w-full">
              {submitting ? "Accesso in corso…" : "Accedi"}
            </Button>
          </form>
        ) : (
          <form onSubmit={handleVerify}>
            <p className="mb-4 text-sm text-muted">
              {mfaMethod === "email"
                ? "Inserisci il codice che ti abbiamo inviato via email."
                : "Inserisci il codice dalla tua app di autenticazione."}
            </p>
            <FieldGroup>
              <Label htmlFor="code">Codice</Label>
              <Input
                id="code"
                inputMode="numeric"
                required
                autoFocus
                value={code}
                onChange={(e) => setCode(e.target.value)}
              />
            </FieldGroup>
            {error && (
              <div className="mb-4">
                <Alert kind="error">{error}</Alert>
              </div>
            )}
            <Button type="submit" disabled={submitting} className="w-full">
              {submitting ? "Verifica…" : "Verifica"}
            </Button>
          </form>
        )}
      </Card>
    </main>
  );
}
