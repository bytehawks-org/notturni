"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { SkeletonRows } from "@/components/ui/States";
import { useToast } from "@/components/ui/Toast";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { LegalDocKey, LegalDocSummary, LegalOverview } from "@/lib/types";

const DOCS: LegalDocKey[] = ["privacy", "cookie"];
const SLUGS: Record<LegalDocKey, string> = { privacy: "privacy", cookie: "cookie-policy" };

/** Dati del titolare, in ordine; `always` = serve sempre, gli altri solo se la funzione corrispondente è attiva. */
const FIELDS: { key: string; always?: boolean; needs?: "backup" | "audit_archive"; wide?: boolean }[] = [
  { key: "controller_name", always: true },
  { key: "controller_address", always: true, wide: true },
  { key: "controller_email", always: true },
  { key: "controller_registry" },
  { key: "dpo_contact", wide: true },
  { key: "hosting_description", always: true, wide: true },
  { key: "processors", wide: true },
  { key: "backup_retention", needs: "backup" },
  { key: "audit_archive_retention", needs: "audit_archive" },
  { key: "minimum_age" },
];

function errorMessage(err: unknown, fallback: string): string {
  return err instanceof ApiClientError ? err.message : fallback;
}

function statusOf(d: LegalDocSummary): "issues" | "missing" | "stale" | "published" | "draft" {
  if (d.issues.length > 0) return "issues";
  if (d.published) return d.stale ? "stale" : "published";
  return d.missing.length > 0 ? "missing" : "draft";
}

const BADGE: Record<ReturnType<typeof statusOf>, string> = {
  published: "bg-ok/15 text-ok",
  stale: "bg-warn/15 text-warn",
  missing: "bg-warn/15 text-warn",
  issues: "bg-danger/15 text-danger",
  draft: "bg-border text-muted",
};

/** Informativa privacy e cookie policy: dati del titolare, funzioni rilevate e stato di ogni documento per lingua.
 * Il testo si modifica nella pagina del singolo documento; qui si completano i dati, si pubblica e si aggiungono
 * lingue. */
export default function AdminLegalPage() {
  const { authFetch } = useAuth();
  const router = useRouter();
  const t = useTranslations("AdminLegal");
  const tc = useTranslations("Common");
  const notify = useToast();
  const [ov, setOv] = useState<LegalOverview | null>(null);
  const [form, setForm] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [newLocale, setNewLocale] = useState<Record<LegalDocKey, string>>({ privacy: "", cookie: "" });

  const apply = useCallback((next: LegalOverview) => {
    setOv(next);
    setForm({ ...next.settings });
  }, []);
  const load = useCallback(() => {
    authFetch((token) => api.legal.overview(token))
      .then(apply)
      .catch((err) => setError(errorMessage(err, tc("unexpectedError"))));
  }, [authFetch, apply, tc]);
  useEffect(load, [load]);

  async function saveSettings(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const age = Number(form.minimum_age);
      const payload: Record<string, string | number | undefined> = Object.fromEntries(
        FIELDS.filter((f) => f.key !== "minimum_age").map((f) => [f.key, form[f.key]?.trim() || undefined])
      );
      payload.minimum_age = form.minimum_age?.trim() ? age : undefined;
      apply(await authFetch((token) => api.legal.saveSettings(token, payload)));
      notify(t("settingsSaved"));
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    } finally {
      setBusy(false);
    }
  }

  async function act(fn: (token: string) => Promise<unknown>, done: string) {
    setBusy(true);
    setError(null);
    try {
      await authFetch(fn);
      notify(done);
      load();
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    } finally {
      setBusy(false);
    }
  }

  async function publish(d: LegalDocSummary) {
    setBusy(true);
    setError(null);
    try {
      try {
        await authFetch((token) => api.legal.publish(token, d.doc, d.locale));
      } catch (err) {
        // esiste già una pagina con lo stesso indirizzo non creata da qui: serve una conferma esplicita
        if (err instanceof ApiClientError && err.status === 409 && window.confirm(`${err.message}\n\n${t("replaceConfirm")}`)) {
          await authFetch((token) => api.legal.publish(token, d.doc, d.locale, true));
        } else {
          throw err;
        }
      }
      notify(t("published"));
      load();
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    } finally {
      setBusy(false);
    }
  }

  async function addLanguage(doc: LegalDocKey) {
    const code = newLocale[doc].trim().toLowerCase();
    if (!/^[a-z]{2}$/.test(code)) {
      setError(t("badLocale"));
      return;
    }
    const base = ov?.documents.find((d) => d.doc === doc && d.has_template)?.locale ?? "it";
    try {
      await authFetch((token) => api.legal.reset(token, doc, code, base));
      router.push(`/admin/legale/${doc}/${code}`);
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    }
  }

  if (!ov) return error ? <Alert kind="error">{error}</Alert> : <SkeletonRows rows={6} />;

  const needed = new Set(FIELDS.filter((f) => f.always || (f.needs && ov.flags[f.needs])).map((f) => f.key));
  const incomplete = ov.documents.filter((d) => d.missing.length > 0 && !d.published).length;
  const activeFlags = Object.entries(ov.flags).filter(([k, v]) => v && !["transfers", "solo"].includes(k));

  return (
    <div className="flex max-w-4xl flex-col gap-6">
      <div>
        <h1 className="font-serif text-2xl text-foreground">{t("title")}</h1>
        <p className="mt-2 text-sm text-muted">{t("intro")}</p>
      </div>
      <Alert kind="info">{t("disclaimer")}</Alert>
      {error && <Alert kind="error">{error}</Alert>}
      {incomplete > 0 && <Alert kind="info">{t("incomplete")}</Alert>}

      <form onSubmit={saveSettings} id="titolare">
        <Card>
          <CardTitle>{t("controllerTitle")}</CardTitle>
          <p className="mt-1 mb-4 text-[13px] text-muted">{t("controllerIntro")}</p>
          <div className="grid gap-x-4 sm:grid-cols-2">
            {FIELDS.map((f) => (
              <FieldGroup key={f.key} className={f.wide ? "sm:col-span-2" : ""}>
                <Label htmlFor={`lg-${f.key}`} hint={needed.has(f.key) ? t("required") : t("optional")}>
                  {t(`fields.${f.key}`)}
                </Label>
                <Input
                  id={`lg-${f.key}`}
                  inputMode={f.key === "minimum_age" ? "numeric" : undefined}
                  maxLength={f.key === "processors" ? 1000 : 500}
                  placeholder={t(`placeholders.${f.key}`)}
                  value={form[f.key] ?? ""}
                  onChange={(e) => setForm((s) => ({ ...s, [f.key]: e.target.value }))}
                />
                <p className="mt-1 text-[12px] text-muted">{t(`help.${f.key}`)}</p>
              </FieldGroup>
            ))}
          </div>
          <Button type="submit" disabled={busy}>
            {busy ? tc("saving") : tc("save")}
          </Button>
        </Card>
      </form>

      <Card>
        <CardTitle>{t("featuresTitle")}</CardTitle>
        <p className="mt-1 mb-3 text-[13px] text-muted">{t("featuresIntro")}</p>
        {activeFlags.length === 0 ? (
          <p className="text-sm text-muted">{t("noFeatures")}</p>
        ) : (
          <ul className="flex flex-wrap gap-2">
            {activeFlags.map(([k]) => (
              <li key={k} className="rounded-full border border-border px-2.5 py-1 text-xs text-foreground">
                {t(`flags.${k}`)}
              </li>
            ))}
          </ul>
        )}
        <dl className="mt-4 grid gap-x-6 gap-y-1 text-[13px] sm:grid-cols-2">
          <dt className="text-muted">{t("values.instance_fqdn")}</dt>
          <dd className="font-mono text-xs">{ov.values.instance_fqdn}</dd>
          <dt className="text-muted">{t("values.session_days")}</dt>
          <dd>{ov.values.session_days}</dd>
          <dt className="text-muted">{t("values.audit_days")}</dt>
          <dd>{ov.values.audit_days}</dd>
        </dl>
      </Card>

      {DOCS.map((doc) => {
        // prima le lingue del modello (it, en), poi le aggiunte in ordine alfabetico
        const rank = (l: string) => (ov.template_locales.includes(l) ? ov.template_locales.indexOf(l) : 99);
        const rows = ov.documents.filter((d) => d.doc === doc).sort((a, b) => rank(a.locale) - rank(b.locale) || a.locale.localeCompare(b.locale));
        return (
          <Card key={doc}>
            <CardTitle>{t(`docs.${doc}`)}</CardTitle>
            <p className="mt-1 mb-3 text-[13px] text-muted">{t(`docsIntro.${doc}`)}</p>
            <ul className="flex flex-col divide-y divide-border">
              {rows.map((d) => {
                const status = statusOf(d);
                return (
                  <li key={d.locale} className="flex flex-wrap items-center gap-x-4 gap-y-2 py-3">
                    <span className="w-10 font-mono text-sm uppercase text-foreground">{d.locale}</span>
                    <span className={`rounded-full px-2.5 py-0.5 text-xs ${BADGE[status]}`}>{t(`status.${status}`)}</span>
                    {d.customized && <span className="text-xs text-muted">{t("customized")}</span>}
                    {d.missing.length > 0 && <span className="text-xs text-muted">{t("missingCount", { count: d.missing.length })}</span>}
                    <span className="ml-auto flex flex-wrap items-center gap-2">
                      <Link href={`/admin/legale/${doc}/${d.locale}`} className="text-[13px] text-primary no-underline hover:underline">
                        {t("edit")}
                      </Link>
                      {d.published && (
                        <Link href={`/p/${SLUGS[doc]}?locale=${d.locale}`} target="_blank" className="text-[13px] text-primary no-underline hover:underline">
                          {t("open")} ↗
                        </Link>
                      )}
                      {(!d.published || d.stale) && (
                        <Button size="sm" disabled={busy || d.missing.length > 0 || d.issues.length > 0} onClick={() => publish(d)}>
                          {d.published ? t("republish") : t("publish")}
                        </Button>
                      )}
                      {d.published && (
                        <Button size="sm" variant="secondary" disabled={busy} onClick={() => act((tk) => api.legal.unpublish(tk, d.doc, d.locale), t("unpublished"))}>
                          {t("unpublish")}
                        </Button>
                      )}
                      {!ov.template_locales.includes(d.locale) && (
                        <Button
                          size="sm"
                          variant="secondary"
                          disabled={busy}
                          onClick={() => window.confirm(t("deleteConfirm", { locale: d.locale })) && act((tk) => api.legal.remove(tk, d.doc, d.locale), t("deleted"))}
                        >
                          {tc("remove")}
                        </Button>
                      )}
                    </span>
                  </li>
                );
              })}
            </ul>
            <div className="mt-4 flex flex-wrap items-center gap-2">
              <Input
                aria-label={t("newLanguage")}
                placeholder={t("newLanguagePlaceholder")}
                className="w-24"
                maxLength={2}
                value={newLocale[doc]}
                onChange={(e) => setNewLocale((s) => ({ ...s, [doc]: e.target.value }))}
              />
              <Button size="sm" variant="secondary" onClick={() => addLanguage(doc)}>
                {t("addLanguage")}
              </Button>
              <span className="text-[12px] text-muted">{t("addLanguageHint")}</span>
            </div>
          </Card>
        );
      })}
    </div>
  );
}
