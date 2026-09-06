"use client";

import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { ApiToken } from "@/lib/types";

function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

export default function DashboardTokenPage() {
  const { authFetch } = useAuth();
  const [tokens, setTokens] = useState<ApiToken[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [name, setName] = useState("");
  const [creating, setCreating] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const [justCreated, setJustCreated] = useState<string | null>(null);

  const load = useCallback(() => {
    authFetch((token) => api.tokens.list(token))
      .then((list) => {
        setTokens(list);
        setError(null);
      })
      .catch((err) => setError(errorMessage(err)));
  }, [authFetch]);

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
    if (!window.confirm("Revocare questo token? Chi lo usa perderà l'accesso all'API.")) return;
    try {
      await authFetch((token) => api.tokens.revoke(token, tokenId));
      load();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  return (
    <div className="space-y-8">
      <div>
        <h1 className="font-serif text-2xl text-foreground">Token API</h1>
        <p className="mt-1 text-sm text-muted">
          Permettono di interfacciarsi con l&apos;API di Notturni senza passare dall&apos;editor o
          dalla dashboard, ad esempio da script o integrazioni esterne.
        </p>
      </div>

      <Card>
        <CardTitle>Nuovo token</CardTitle>
        <form onSubmit={handleCreate} className="flex items-end gap-3">
          <div className="flex-1">
            <FieldGroup>
              <Label htmlFor="token-name">Nome</Label>
              <Input
                id="token-name"
                required
                placeholder="es. integrazione-newsletter"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </FieldGroup>
          </div>
          <Button type="submit" disabled={creating}>
            Crea
          </Button>
        </form>
        {createError && (
          <div className="mt-3">
            <Alert kind="error">{createError}</Alert>
          </div>
        )}
        {justCreated && (
          <div className="mt-4 space-y-2">
            <p className="break-all rounded-md border border-border bg-foreground/5 p-3 font-mono text-xs">
              {justCreated}
            </p>
            <Alert kind="info">
              Questo è l&apos;unico momento in cui il valore completo viene mostrato: copialo ora,
              non potrà essere recuperato in seguito.
            </Alert>
          </div>
        )}
      </Card>

      <Card>
        <CardTitle>Token attivi</CardTitle>
        {error && <Alert kind="error">{error}</Alert>}
        <div className="overflow-x-auto rounded-lg border border-border">
          <table className="w-full text-left text-sm">
            <thead className="border-b border-border text-muted">
              <tr>
                <th className="px-4 py-3">Nome</th>
                <th className="px-4 py-3">Prefisso</th>
                <th className="px-4 py-3 whitespace-nowrap">Creato il</th>
                <th className="px-4 py-3 whitespace-nowrap">Ultimo utilizzo</th>
                <th className="px-4 py-3" />
              </tr>
            </thead>
            <tbody>
              {tokens
                ?.filter((t) => !t.revoked_at)
                .map((t) => (
                  <tr key={t.id} className="border-b border-border align-top last:border-0">
                    <td className="px-4 py-3 text-foreground">{t.name}</td>
                    <td className="px-4 py-3 font-mono text-xs text-muted">{t.token_prefix}…</td>
                    <td className="px-4 py-3 whitespace-nowrap text-muted">
                      {new Date(t.created_at).toLocaleString("it-IT")}
                    </td>
                    <td className="px-4 py-3 whitespace-nowrap text-muted">
                      {t.last_used_at ? new Date(t.last_used_at).toLocaleString("it-IT") : "Mai"}
                    </td>
                    <td className="px-4 py-3 text-right">
                      <Button variant="danger" onClick={() => handleRevoke(t.id)}>
                        Revoca
                      </Button>
                    </td>
                  </tr>
                ))}
              {tokens !== null && tokens.filter((t) => !t.revoked_at).length === 0 && (
                <tr>
                  <td colSpan={5} className="px-4 py-6 text-center text-muted">
                    Nessun token attivo.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </Card>
    </div>
  );
}
