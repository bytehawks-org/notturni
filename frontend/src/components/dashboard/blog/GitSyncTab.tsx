"use client";

import { useLocale, useTranslations } from "next-intl";
import { useCallback, useEffect, useRef, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { Toggle } from "@/components/ui/Controls";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { SkeletonRows } from "@/components/ui/States";
import { useToast } from "@/components/ui/Toast";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDateTime } from "@/lib/format";
import type { GitSyncStatus } from "@/lib/types";

import { errorMessage } from "./shared";

const POLL_MS = 3000;
const POLL_MAX_TICKS = 40;

/** Sincronizzazione Git dello Spazio (solo proprietario): i testi pubblicati vengono esportati come
 * Markdown in un branch dedicato di un repository remoto. Due modi di accesso: token personale via HTTPS
 * (si invia una volta e non viene più mostrato) oppure chiave di deploy SSH generata dalla piattaforma
 * (si mostra solo la parte pubblica, insieme alle impronte dell'host a cui ci si fida). */
export function GitSyncTab({ blogSlug }: { blogSlug: string }) {
  const { authFetch } = useAuth();
  const t = useTranslations("GitSyncTab");
  const tc = useTranslations("Common");
  const locale = useLocale();
  const notify = useToast();
  const [status, setStatus] = useState<GitSyncStatus | null>(null);
  const [remoteUrl, setRemoteUrl] = useState("");
  const [branch, setBranch] = useState("");
  const [username, setUsername] = useState("");
  const [method, setMethod] = useState<"https" | "ssh">("https");
  const [hostFingerprint, setHostFingerprint] = useState("");
  const [secret, setSecret] = useState("");
  const [includeDrafts, setIncludeDrafts] = useState(false);
  const [enabled, setEnabled] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [waiting, setWaiting] = useState(false);
  const ticks = useRef(0);

  const apply = useCallback((s: GitSyncStatus) => {
    setStatus(s);
    setRemoteUrl(s.remote_url ?? "");
    setBranch(s.branch ?? "");
    setUsername(s.username ?? "");
    setMethod(s.auth_method ?? "https");
    setIncludeDrafts(s.include_drafts);
    setEnabled(s.configured ? s.enabled : true);
  }, []);

  const load = useCallback(() => {
    authFetch((token) => api.gitSync.get(token, blogSlug))
      .then(apply)
      .catch((err) => setError(errorMessage(err, tc("unexpectedError"))));
  }, [authFetch, blogSlug, apply, tc]);
  useEffect(load, [load]);

  // dopo «Sincronizza ora» (o con un giro in corso) si ricontrolla lo stato finché non si conclude
  const running = status?.last_status === "running";
  useEffect(() => {
    if (!waiting && !running) return;
    const timer = setInterval(() => {
      ticks.current += 1;
      authFetch((token) => api.gitSync.get(token, blogSlug))
        .then((s) => {
          setStatus(s);
          if (s.last_status !== "running" || ticks.current >= POLL_MAX_TICKS) setWaiting(false);
        })
        .catch(() => setWaiting(false));
    }, POLL_MS);
    return () => clearInterval(timer);
  }, [waiting, running, authFetch, blogSlug]);

  async function save(event: FormEvent, regenerateKey = false) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const updated = await authFetch((token) =>
        api.gitSync.save(token, blogSlug, {
          remote_url: remoteUrl.trim(),
          branch: branch.trim() || undefined,
          auth_method: method,
          username: method === "https" ? username.trim() || undefined : undefined,
          secret: method === "https" ? secret || undefined : undefined,
          regenerate_key: regenerateKey || undefined,
          host_fingerprint: method === "ssh" ? hostFingerprint.trim() || undefined : undefined,
          include_drafts: includeDrafts,
          enabled,
        })
      );
      apply(updated);
      setSecret("");
      setHostFingerprint("");
      notify(t("saved"));
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    } finally {
      setBusy(false);
    }
  }

  async function copyPublicKey() {
    try {
      await navigator.clipboard.writeText(status?.ssh_public_key ?? "");
      notify(t("copied"));
    } catch {
      // clipboard non disponibile: la chiave resta selezionabile a mano
    }
  }

  async function runNow() {
    setBusy(true);
    setError(null);
    try {
      await authFetch((token) => api.gitSync.run(token, blogSlug));
      ticks.current = 0;
      setStatus((s) => (s ? { ...s, last_status: "running" } : s));
      setWaiting(true);
      notify(t("queued"));
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    setConfirmRemove(false);
    setBusy(true);
    try {
      await authFetch((token) => api.gitSync.remove(token, blogSlug));
      setSecret("");
      load();
      notify(t("removed"));
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    } finally {
      setBusy(false);
    }
  }

  if (!status) return error ? <Alert kind="error">{error}</Alert> : <SkeletonRows rows={4} />;

  return (
    <div className="flex max-w-2xl flex-col gap-6">
      <p className="text-sm text-muted">{t("intro")}</p>

      {status.configured && (
        <Card>
          <CardTitle>{t("statusTitle")}</CardTitle>
          <dl className="mt-3 grid gap-x-6 gap-y-1.5 text-sm sm:grid-cols-[150px_minmax(0,1fr)]">
            <dt className="text-muted">{t("lastStatus")}</dt>
            <dd className="font-medium text-foreground">
              {status.last_status ? t(`status.${status.last_status}`) : t("neverRun")}
            </dd>
            {status.last_run_at && (
              <>
                <dt className="text-muted">{t("lastRun")}</dt>
                <dd>{formatDateTime(status.last_run_at, locale)}</dd>
              </>
            )}
            {status.last_pages !== null && (
              <>
                <dt className="text-muted">{t("pages")}</dt>
                <dd>{status.last_pages}</dd>
              </>
            )}
            {status.last_commit && (
              <>
                <dt className="text-muted">{t("commit")}</dt>
                <dd className="font-mono text-xs">{status.last_commit.slice(0, 12)}</dd>
              </>
            )}
          </dl>
          {status.last_status === "error" && status.last_error && (
            <div className="mt-3">
              <Alert kind="error">{status.last_error}</Alert>
            </div>
          )}
          <div className="mt-4">
            <Button variant="secondary" size="sm" onClick={runNow} disabled={busy || running || !status.enabled}>
              {running ? t("running") : t("runNow")}
            </Button>
          </div>
        </Card>
      )}

      <form onSubmit={save}>
        <Card>
          <CardTitle>{status.configured ? t("settingsTitle") : t("setupTitle")}</CardTitle>
          <div className="mt-4">
            <FieldGroup>
              <Label>{t("method")}</Label>
              <div className="grid gap-2.5 sm:grid-cols-2">
                {(["https", "ssh"] as const).map((m) => (
                  <button
                    key={m}
                    type="button"
                    aria-pressed={method === m}
                    onClick={() => setMethod(m)}
                    className={`flex flex-col gap-0.5 rounded-lg border px-3.5 py-3 text-left transition ${method === m ? "border-primary bg-primary/5" : "border-border hover:border-primary/40"}`}
                  >
                    <span className="text-sm font-semibold text-foreground">{t(`methods.${m}`)}</span>
                    <span className="text-[13px] text-muted">{t(`methods.${m}Sub`)}</span>
                  </button>
                ))}
              </div>
            </FieldGroup>
            <FieldGroup>
              <Label htmlFor="git-url" hint={method === "ssh" ? t("urlHintSsh") : t("urlHint")}>
                {t("url")}
              </Label>
              <Input
                id="git-url"
                type={method === "ssh" ? "text" : "url"}
                required
                placeholder={method === "ssh" ? "git@github.com:utente/testi.git" : "https://github.com/utente/testi.git"}
                value={remoteUrl}
                onChange={(e) => setRemoteUrl(e.target.value)}
              />
            </FieldGroup>
            <FieldGroup>
              <Label htmlFor="git-branch" hint={t("branchHint")}>
                {t("branch")}
              </Label>
              <Input id="git-branch" placeholder="notturni-backup" value={branch} onChange={(e) => setBranch(e.target.value)} />
            </FieldGroup>
            {method === "https" ? (
              <>
                <FieldGroup>
                  <Label htmlFor="git-user" hint={t("usernameHint")}>
                    {t("username")}
                  </Label>
                  <Input id="git-user" placeholder="git" value={username} onChange={(e) => setUsername(e.target.value)} />
                </FieldGroup>
                <FieldGroup>
                  <Label htmlFor="git-token" hint={status.has_secret && status.auth_method === "https" ? t("tokenSet") : t("tokenHint")}>
                    {t("token")}
                  </Label>
                  <Input
                    id="git-token"
                    type="password"
                    autoComplete="off"
                    required={!(status.has_secret && status.auth_method === "https")}
                    placeholder={status.has_secret && status.auth_method === "https" ? "••••••••" : ""}
                    value={secret}
                    onChange={(e) => setSecret(e.target.value)}
                  />
                </FieldGroup>
              </>
            ) : (
              <>
                {status.auth_method === "ssh" && status.ssh_public_key ? (
                  <FieldGroup>
                    <Label hint={t("deployKeyHint")}>{t("deployKey")}</Label>
                    <textarea
                      readOnly
                      rows={3}
                      value={status.ssh_public_key}
                      onFocus={(e) => e.currentTarget.select()}
                      className="w-full rounded-lg border border-border bg-surface px-3 py-2.5 font-mono text-xs text-foreground"
                    />
                    <div className="mt-2 flex flex-wrap gap-2">
                      <Button type="button" variant="secondary" size="sm" onClick={copyPublicKey}>
                        {t("copy")}
                      </Button>
                      <Button type="button" variant="secondary" size="sm" disabled={busy} onClick={(e) => save(e, true)}>
                        {t("regenerate")}
                      </Button>
                    </div>
                  </FieldGroup>
                ) : (
                  <p className="mb-4 text-sm text-muted">{t("keyOnSave")}</p>
                )}
                {status.auth_method === "ssh" && status.ssh_host_fingerprints.length > 0 && (
                  <FieldGroup>
                    <Label hint={t("hostKeyHint")}>{t("hostKey")}</Label>
                    <ul className="flex flex-col gap-1 font-mono text-xs text-foreground">
                      {status.ssh_host_fingerprints.map((f) => (
                        <li key={f} className="break-all">
                          {f}
                        </li>
                      ))}
                    </ul>
                  </FieldGroup>
                )}
                <FieldGroup>
                  <Label htmlFor="git-fp" hint={t("expectedFingerprintHint")}>
                    {t("expectedFingerprint")}
                  </Label>
                  <Input
                    id="git-fp"
                    placeholder="SHA256:…"
                    value={hostFingerprint}
                    onChange={(e) => setHostFingerprint(e.target.value)}
                  />
                </FieldGroup>
              </>
            )}
            <div className="mb-4 flex flex-col gap-3 rounded-xl border border-border bg-surface px-4 py-3">
              <Toggle checked={includeDrafts} onChange={setIncludeDrafts} label={t("includeDrafts")} />
              <span className="-mt-2 text-[13px] text-muted">{t("includeDraftsSub")}</span>
              <Toggle checked={enabled} onChange={setEnabled} label={t("enabled")} />
            </div>
            {error && (
              <div className="mb-4">
                <Alert kind="error">{error}</Alert>
              </div>
            )}
            <div className="flex flex-wrap items-center gap-3">
              <Button type="submit" disabled={busy}>
                {busy ? t("saving") : t("save")}
              </Button>
              {status.configured && (
                <Button type="button" variant="secondary" onClick={() => setConfirmRemove(true)} disabled={busy}>
                  {t("remove")}
                </Button>
              )}
            </div>
          </div>
        </Card>
      </form>

      <Alert kind="info">{t("privacyNote")}</Alert>

      <ConfirmDialog
        open={confirmRemove}
        title={t("removeTitle")}
        body={t("removeBody")}
        confirmText={blogSlug}
        confirmLabel={t("remove")}
        onCancel={() => setConfirmRemove(false)}
        onConfirm={remove}
      />
    </div>
  );
}
