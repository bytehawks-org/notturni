"use client";

import { useLocale, useTranslations } from "next-intl";
import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { FieldGroup, Input, Label, TextArea } from "@/components/ui/Field";
import { SkeletonRows } from "@/components/ui/States";
import { useToast } from "@/components/ui/Toast";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDateTime } from "@/lib/format";
import type { KnownHost } from "@/lib/types";

function errorMessage(err: unknown, fallback: string): string {
  return err instanceof ApiClientError ? err.message : fallback;
}

const lines = (text: string) =>
  text
    .split(/\s+/)
    .map((l) => l.trim())
    .filter(Boolean);

/** Impronte attese delle host key SSH dei provider (solo Super Admin): la sincronizzazione Git fissa solo le
 * chiavi rilevate che hanno una di queste impronte. Si aggiornano quando un provider ruota le chiavi. */
export default function AdminGitSyncPage() {
  const { authFetch } = useAuth();
  const t = useTranslations("AdminGitSync");
  const tc = useTranslations("Common");
  const locale = useLocale();
  const notify = useToast();
  const [hosts, setHosts] = useState<KnownHost[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [host, setHost] = useState("");
  const [port, setPort] = useState("22");
  const [prints, setPrints] = useState("");
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [removing, setRemoving] = useState<KnownHost | null>(null);

  const load = useCallback(() => {
    authFetch((token) => api.gitSync.knownHosts.list(token))
      .then(setHosts)
      .catch((err) => setError(errorMessage(err, tc("unexpectedError"))));
  }, [authFetch, tc]);
  useEffect(load, [load]);

  function edit(h: KnownHost | null) {
    setEditing(h ? `${h.host}:${h.port}` : null);
    setHost(h?.host ?? "");
    setPort(String(h?.port ?? 22));
    setPrints(h?.fingerprints.join("\n") ?? "");
    setNote(h?.note ?? "");
    setError(null);
  }

  async function save(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await authFetch((token) =>
        api.gitSync.knownHosts.save(token, {
          host: host.trim(),
          port: Number(port) || 22,
          fingerprints: lines(prints),
          note: note.trim() || undefined,
        })
      );
      notify(t("saved"));
      edit(null);
      load();
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    } finally {
      setBusy(false);
    }
  }

  async function remove(h: KnownHost) {
    setRemoving(null);
    try {
      await authFetch((token) => api.gitSync.knownHosts.remove(token, h.host, h.port));
      notify(t("removed"));
      load();
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    }
  }

  return (
    <div className="flex max-w-3xl flex-col gap-6">
      <div>
        <h1 className="font-serif text-2xl text-foreground">{t("title")}</h1>
        <p className="mt-2 text-sm text-muted">{t("intro")}</p>
      </div>

      {error && <Alert kind="error">{error}</Alert>}

      {hosts === null ? (
        <SkeletonRows rows={3} />
      ) : (
        <div className="flex flex-col gap-3">
          {hosts.length === 0 && <p className="text-sm text-muted">{t("empty")}</p>}
          {hosts.map((h) => (
            <Card key={`${h.host}:${h.port}`}>
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div className="min-w-0">
                  <CardTitle>
                    {h.host}
                    {h.port !== 22 && <span className="text-muted">:{h.port}</span>}
                  </CardTitle>
                  {h.note && <p className="mt-1 text-[13px] text-muted">{h.note}</p>}
                  <ul className="mt-2 flex flex-col gap-1 font-mono text-xs text-foreground">
                    {h.fingerprints.map((f) => (
                      <li key={f} className="break-all">
                        {f}
                      </li>
                    ))}
                  </ul>
                  <p className="mt-2 text-[12px] text-muted">{t("updated", { date: formatDateTime(h.updated_at, locale) })}</p>
                </div>
                <div className="flex gap-2">
                  <Button variant="secondary" size="sm" onClick={() => edit(h)}>
                    {tc("edit")}
                  </Button>
                  <Button variant="secondary" size="sm" onClick={() => setRemoving(h)}>
                    {tc("remove")}
                  </Button>
                </div>
              </div>
            </Card>
          ))}
        </div>
      )}

      <form onSubmit={save}>
        <Card>
          <CardTitle>{editing ? t("editTitle", { host: editing }) : t("addTitle")}</CardTitle>
          <div className="mt-4">
            <div className="grid gap-4 sm:grid-cols-[minmax(0,1fr)_110px]">
              <FieldGroup>
                <Label htmlFor="kh-host">{t("host")}</Label>
                <Input id="kh-host" required placeholder="github.com" value={host} onChange={(e) => setHost(e.target.value)} />
              </FieldGroup>
              <FieldGroup>
                <Label htmlFor="kh-port">{t("port")}</Label>
                <Input id="kh-port" inputMode="numeric" value={port} onChange={(e) => setPort(e.target.value)} />
              </FieldGroup>
            </div>
            <FieldGroup>
              <Label htmlFor="kh-fp" hint={t("fingerprintsHint")}>
                {t("fingerprints")}
              </Label>
              <TextArea
                id="kh-fp"
                required
                rows={4}
                className="font-mono text-xs"
                placeholder="SHA256:…"
                value={prints}
                onChange={(e) => setPrints(e.target.value)}
              />
            </FieldGroup>
            <FieldGroup>
              <Label htmlFor="kh-note">{t("note")}</Label>
              <Input id="kh-note" maxLength={500} value={note} onChange={(e) => setNote(e.target.value)} />
            </FieldGroup>
            <div className="flex gap-3">
              <Button type="submit" disabled={busy}>
                {busy ? tc("saving") : tc("save")}
              </Button>
              {editing && (
                <Button type="button" variant="secondary" onClick={() => edit(null)}>
                  {tc("cancel")}
                </Button>
              )}
            </div>
          </div>
        </Card>
      </form>

      <Alert kind="info">{t("rotationNote")}</Alert>

      <ConfirmDialog
        open={removing !== null}
        title={t("removeTitle")}
        body={t("removeBody")}
        confirmText={removing?.host ?? ""}
        confirmLabel={tc("remove")}
        onCancel={() => setRemoving(null)}
        onConfirm={() => removing && remove(removing)}
      />
    </div>
  );
}
