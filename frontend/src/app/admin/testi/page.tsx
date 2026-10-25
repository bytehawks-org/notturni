"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";
import { useCallback, useEffect, useMemo, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Input, TextArea } from "@/components/ui/Field";
import { FilterChip } from "@/components/ui/Pill";
import { SkeletonRows } from "@/components/ui/States";
import { useToast } from "@/components/ui/Toast";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { LOCALES, LOCALE_NAMES, type Locale } from "@/i18n/config";
import { TEXT_GROUPS, editableEntries, isPlainText } from "@/lib/site-texts";

/** `pillars.noAi.title` → «pillars › no ai › title»: un'etichetta leggibile al posto del percorso del messaggio. */
const humanize = (path: string) =>
  path
    .split(".")
    .map((part) => part.replace(/([a-z])([A-Z])/g, "$1 $2").toLowerCase())
    .join(" › ");

function errorMessage(err: unknown, fallback: string): string {
  return err instanceof ApiClientError ? err.message : fallback;
}

/** Testi dell'interfaccia riscrivibili dagli amministratori, per lingua: home (manifesto, pilastri, pulsanti) e
 * modulo di iscrizione alla newsletter. Il testo predefinito resta sotto ogni campo; svuotare un campo (o
 * «Ripristina») riporta il predefinito. */
export default function AdminTextsPage() {
  const { authFetch } = useAuth();
  const t = useTranslations("AdminTexts");
  const tc = useTranslations("Common");
  const notify = useToast();
  const [locale, setLocale] = useState<Locale>("it");
  const [groupId, setGroupId] = useState<(typeof TEXT_GROUPS)[number]["id"]>("home");
  const [messages, setMessages] = useState<Record<string, never> | null>(null);
  const [overrides, setOverrides] = useState<Record<string, string>>({});
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    Promise.all([import(`@/messages/${locale}.json`), api.siteTexts.get(locale)])
      .then(([m, current]) => {
        setMessages(m.default as Record<string, never>);
        setOverrides(current.texts);
      })
      .catch((err) => setError(errorMessage(err, tc("unexpectedError"))));
  }, [locale, tc]);
  useEffect(load, [load]);

  function changeLocale(next: Locale) {
    if (next === locale) return;
    setMessages(null);
    setDrafts({});
    setError(null);
    setLocale(next);
  }

  const group = TEXT_GROUPS.find((g) => g.id === groupId)!;
  const entries = useMemo(() => (messages ? editableEntries(messages, group.namespace) : []), [messages, group]);
  const valueOf = (key: string) => drafts[key] ?? overrides[key] ?? "";
  const dirty = Object.keys(drafts).filter((k) => drafts[k].trim() !== (overrides[k] ?? ""));

  async function save() {
    const invalid = dirty.find((k) => drafts[k].trim() !== "" && !isPlainText(drafts[k].trim(), k));
    if (invalid) {
      setError(t("invalid", { key: invalid, chars: "{ } < >", tag: "<privacy>" }));
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const payload = Object.fromEntries(dirty.map((k) => [k, drafts[k].trim() === "" ? null : drafts[k].trim()]));
      const saved = await authFetch((token) => api.siteTexts.save(token, locale, payload));
      setOverrides(saved.texts);
      setDrafts({});
      notify(t("saved"));
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex max-w-3xl flex-col gap-6">
      <div>
        <h1 className="font-serif text-2xl text-foreground">{t("title")}</h1>
        <p className="mt-2 text-sm text-muted">{t("intro")}</p>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        {LOCALES.map((l) => (
          <FilterChip key={l} active={locale === l} onClick={() => changeLocale(l)}>
            {LOCALE_NAMES[l]}
          </FilterChip>
        ))}
        <span className="mx-1 h-5 w-px bg-border" aria-hidden="true" />
        {TEXT_GROUPS.map((g) => (
          <FilterChip key={g.id} active={groupId === g.id} onClick={() => setGroupId(g.id)}>
            {t(`groups.${g.id}`)}
          </FilterChip>
        ))}
        <Link href="/" className="ml-auto text-sm text-primary no-underline hover:underline" target="_blank">
          {t("preview")} ↗
        </Link>
      </div>

      {error && <Alert kind="error">{error}</Alert>}

      {messages === null ? (
        <SkeletonRows rows={5} />
      ) : (
        <div className="flex flex-col gap-3">
          {entries.map(({ key, defaultText }) => {
            const value = valueOf(key);
            const custom = overrides[key] !== undefined;
            const changed = dirty.includes(key);
            const Field = defaultText.length > 70 ? TextArea : Input;
            return (
              <Card key={key}>
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <label htmlFor={`txt-${key}`} className="font-mono text-[12px] text-muted">
                    {humanize(key.slice(group.namespace.length + 1))}
                  </label>
                  <div className="flex items-center gap-2 text-[12px]">
                    {changed ? (
                      <span className="text-primary">{t("unsaved")}</span>
                    ) : (
                      custom && <span className="text-muted">{t("customized")}</span>
                    )}
                    {(custom || value !== "") && (
                      <button
                        type="button"
                        className="text-muted underline hover:text-foreground"
                        onClick={() => setDrafts((d) => ({ ...d, [key]: "" }))}
                      >
                        {t("restore")}
                      </button>
                    )}
                  </div>
                </div>
                <Field
                  id={`txt-${key}`}
                  className="mt-2 min-h-14!"
                  rows={2}
                  maxLength={2000}
                  placeholder={defaultText}
                  value={value}
                  onChange={(e) => setDrafts((d) => ({ ...d, [key]: e.target.value }))}
                />
                <p className="mt-1.5 text-[12px] text-muted">
                  {t("default")}: {defaultText}
                </p>
              </Card>
            );
          })}
          <p className="text-[12px] text-muted">{t("limits", { chars: "{ } < >", tag: "<privacy>…</privacy>" })}</p>
        </div>
      )}

      <div className="sticky bottom-4 flex items-center gap-3 rounded-xl border border-border bg-surface px-4 py-3">
        <Button onClick={save} disabled={busy || dirty.length === 0}>
          {busy ? tc("saving") : t("saveN", { count: dirty.length })}
        </Button>
        {dirty.length > 0 && (
          <Button variant="secondary" onClick={() => setDrafts({})} disabled={busy}>
            {tc("cancel")}
          </Button>
        )}
      </div>
    </div>
  );
}
