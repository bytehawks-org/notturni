"use client";

import MarkdownIt from "markdown-it";
import { useTranslations } from "next-intl";
import Link from "next/link";
import { useParams, useRouter } from "next/navigation";
import { useCallback, useEffect, useMemo, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label, TextArea } from "@/components/ui/Field";
import { SkeletonRows } from "@/components/ui/States";
import { useToast } from "@/components/ui/Toast";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { LegalDoc, LegalDocKey, LegalOverview } from "@/lib/types";

const SLUGS: Record<LegalDocKey, string> = { privacy: "privacy", cookie: "cookie-policy" };

function errorMessage(err: unknown, fallback: string): string {
  return err instanceof ApiClientError ? err.message : fallback;
}

/** Editor di un documento legale in una lingua: sorgente Markdown con `{{variabili}}` e `{{#if funzione}}`,
 * anteprima del testo risolto (quello che si pubblica) e stato di completezza. */
export default function LegalDocEditorPage() {
  const params = useParams<{ doc: string; locale: string }>();
  const doc = params.doc as LegalDocKey;
  const locale = params.locale;
  const router = useRouter();
  const { authFetch } = useAuth();
  const t = useTranslations("AdminLegal");
  const tc = useTranslations("Common");
  const notify = useToast();
  const [data, setData] = useState<LegalDoc | null>(null);
  const [overview, setOverview] = useState<LegalOverview | null>(null);
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [copyFrom, setCopyFrom] = useState("");

  const apply = useCallback((d: LegalDoc) => {
    setData(d);
    setTitle(d.title);
    setContent(d.content);
  }, []);
  useEffect(() => {
    let cancelled = false;
    Promise.all([authFetch((tk) => api.legal.doc(tk, doc, locale)), authFetch((tk) => api.legal.overview(tk))])
      .then(([d, ov]) => {
        if (cancelled) return;
        apply(d);
        setOverview(ov);
      })
      .catch((err) => !cancelled && setError(errorMessage(err, tc("unexpectedError"))));
    return () => {
      cancelled = true;
    };
  }, [authFetch, doc, locale, apply, tc]);

  const md = useMemo(() => new MarkdownIt({ html: false, linkify: true }), []);
  const preview = useMemo(() => (data ? md.render(`# ${data.title}\n\n${data.rendered}`) : ""), [md, data]);
  const dirty = data !== null && (title !== data.title || content !== data.content);

  async function run(fn: (token: string) => Promise<LegalDoc>, done: string) {
    setBusy(true);
    setError(null);
    try {
      apply(await authFetch(fn));
      notify(done);
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    } finally {
      setBusy(false);
    }
  }

  async function publish() {
    if (dirty) {
      setError(t("saveFirst"));
      return;
    }
    setBusy(true);
    setError(null);
    try {
      try {
        apply(await authFetch((tk) => api.legal.publish(tk, doc, locale)));
      } catch (err) {
        if (err instanceof ApiClientError && err.status === 409 && window.confirm(`${err.message}\n\n${t("replaceConfirm")}`)) {
          apply(await authFetch((tk) => api.legal.publish(tk, doc, locale, true)));
        } else {
          throw err;
        }
      }
      notify(t("published"));
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    } finally {
      setBusy(false);
    }
  }

  if (!data) return error ? <Alert kind="error">{error}</Alert> : <SkeletonRows rows={8} />;

  const others = (overview?.documents ?? []).filter((d) => d.doc === doc && d.locale !== locale);
  const flags = overview ? Object.keys(overview.flags).sort() : [];

  return (
    <div className="flex flex-col gap-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <Link href="/admin/legale" className="text-[13px] text-muted no-underline hover:text-foreground">
            ← {t("title")}
          </Link>
          <h1 className="font-serif text-2xl text-foreground">
            {t(`docs.${doc}`)} · <span className="uppercase">{locale}</span>
          </h1>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          {data.published && (
            <Link href={`/p/${SLUGS[doc]}?locale=${locale}`} target="_blank" className="text-[13px] text-primary no-underline hover:underline">
              {t("open")} ↗
            </Link>
          )}
          <Button variant="secondary" onClick={() => run((tk) => api.legal.saveDoc(tk, doc, locale, { title, content }), t("saved"))} disabled={busy || !dirty}>
            {busy ? tc("saving") : t("saveText")}
          </Button>
          {(!data.published || data.stale) && (
            <Button onClick={publish} disabled={busy || data.missing.length > 0 || data.issues.length > 0}>
              {data.published ? t("republish") : t("publish")}
            </Button>
          )}
          {data.published && (
            <Button variant="secondary" disabled={busy} onClick={() => run((tk) => api.legal.unpublish(tk, doc, locale), t("unpublished"))}>
              {t("unpublish")}
            </Button>
          )}
        </div>
      </div>

      {error && <Alert kind="error">{error}</Alert>}
      {data.missing.length > 0 && (
        <Alert kind="info">
          {t("missingTitle")}: <span className="font-mono text-xs">{data.missing.join(", ")}</span>.{" "}
          <Link href="/admin/legale#titolare" className="underline">
            {t("completeData")}
          </Link>
        </Alert>
      )}
      {data.issues.length > 0 && (
        <Alert kind="error">
          {t("issuesTitle")}: {data.issues.join("; ")}
        </Alert>
      )}
      {data.stale && <Alert kind="info">{t("staleNotice")}</Alert>}
      {dirty && <Alert kind="info">{t("unsaved")}</Alert>}

      <div className="grid gap-5 lg:grid-cols-2">
        <Card>
          <FieldGroup>
            <Label htmlFor="lg-title">{t("docTitle")}</Label>
            <Input id="lg-title" maxLength={255} value={title} onChange={(e) => setTitle(e.target.value)} />
          </FieldGroup>
          <FieldGroup>
            <Label htmlFor="lg-content" hint={t("sourceHint", { tag: "{{…}}" })}>
              {t("source")}
            </Label>
            <TextArea
              id="lg-content"
              rows={30}
              spellCheck={false}
              className="font-mono text-[13px] leading-relaxed"
              value={content}
              onChange={(e) => setContent(e.target.value)}
            />
          </FieldGroup>
          <div className="flex flex-wrap items-center gap-3">
            {data.has_template ? (
              <Button
                variant="secondary"
                size="sm"
                disabled={busy || !data.customized}
                onClick={() => window.confirm(t("resetConfirm")) && run((tk) => api.legal.reset(tk, doc, locale), t("resetDone"))}
              >
                {t("resetTemplate")}
              </Button>
            ) : (
              others.length > 0 && (
                <span className="flex items-center gap-2">
                  <select
                    aria-label={t("copyFrom")}
                    value={copyFrom}
                    onChange={(e) => setCopyFrom(e.target.value)}
                    className="rounded-lg border border-border bg-surface px-2.5 py-1.5 text-xs text-foreground"
                  >
                    <option value="">{t("copyFrom")}</option>
                    {others.map((o) => (
                      <option key={o.locale} value={o.locale}>
                        {o.locale.toUpperCase()}
                      </option>
                    ))}
                  </select>
                  <Button
                    variant="secondary"
                    size="sm"
                    disabled={busy || !copyFrom}
                    onClick={() => window.confirm(t("copyConfirm", { locale: copyFrom })) && run((tk) => api.legal.reset(tk, doc, locale, copyFrom), t("copyDone"))}
                  >
                    {t("copyAction")}
                  </Button>
                </span>
              )
            )}
            {!data.has_template && (
              <Button variant="secondary" size="sm" disabled={busy} onClick={() => router.push("/admin/legale")}>
                {tc("back")}
              </Button>
            )}
          </div>
        </Card>

        <Card>
          <CardTitle>{t("preview")}</CardTitle>
          <p className="mt-1 mb-3 text-[12px] text-muted">{t("previewHint")}</p>
          <div className="notturni-prose max-h-[75vh] overflow-y-auto text-[15px] leading-relaxed" dangerouslySetInnerHTML={{ __html: preview }} />
        </Card>
      </div>

      <details className="rounded-xl border border-border bg-surface px-5 py-4 text-sm">
        <summary className="cursor-pointer font-medium text-foreground">{t("syntaxTitle")}</summary>
        <div className="mt-3 flex flex-col gap-3 text-[13px] text-muted">
          <p>{t("syntaxIntro")}</p>
          <ul className="flex flex-col gap-1 font-mono text-xs text-foreground">
            <li>{"{{controller_name}}"}</li>
            <li>{"{{minimum_age|14}}"}</li>
            <li>{"{{#if newsletter}} … {{/if}}"}</li>
            <li>{"{{#unless dpo_contact}} … {{/unless}}"}</li>
          </ul>
          <p>{t("syntaxFlags", { tag: "{{#if …}}" })}</p>
          <ul className="flex flex-wrap gap-2">
            {flags.map((f) => (
              <li key={f} className={`rounded-full border px-2 py-0.5 font-mono text-[11px] ${overview?.flags[f] ? "border-primary text-foreground" : "border-border"}`}>
                {f}
              </li>
            ))}
          </ul>
          <p>{t("syntaxValues")}</p>
          <p className="font-mono text-[11px] text-foreground">
            controller_name controller_address controller_email controller_registry dpo_contact hosting_description processors backup_retention audit_archive_retention
            minimum_age instance_fqdn session_days audit_days deletion_days grace_days last_update
          </p>
        </div>
      </details>
    </div>
  );
}
