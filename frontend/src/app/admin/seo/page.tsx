"use client";

import { useTranslations } from "next-intl";
import { useCallback, useEffect, useRef, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { Toggle } from "@/components/ui/Controls";
import { FieldGroup, Input, Label, TextArea } from "@/components/ui/Field";
import { SkeletonRows } from "@/components/ui/States";
import { useToast } from "@/components/ui/Toast";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { LOCALES, LOCALE_NAMES, isLocale } from "@/i18n/config";
import { SITE_URL } from "@/lib/site";
import type { SeoAdmin, SeoLocaleText } from "@/lib/types";

const TITLE_HINT = 60;
const DESCRIPTION_HINT = 160;

type LocaleForm = { title: string; description: string; keywords: string };
const emptyLocale: LocaleForm = { title: "", description: "", keywords: "" };

function errorMessage(err: unknown, fallback: string): string {
  return err instanceof ApiClientError ? err.message : fallback;
}

/** Immagine con anteprima, caricamento e rimozione: icona del portale e immagine per le condivisioni. */
function ImageSlot({
  title,
  hint,
  previews,
  busy,
  onUpload,
  onRemove,
  wide,
}: {
  title: string;
  hint: string;
  previews: { label: string; url: string }[];
  busy: boolean;
  onUpload: (file: File) => void;
  onRemove: () => void;
  wide?: boolean;
}) {
  const t = useTranslations("AdminSeo");
  const input = useRef<HTMLInputElement>(null);
  return (
    <div className="flex flex-col gap-3">
      <div>
        <h3 className="text-sm font-semibold text-foreground">{title}</h3>
        <p className="text-[12px] text-muted">{hint}</p>
      </div>
      {previews.length > 0 ? (
        <ul className="flex flex-wrap items-end gap-4">
          {previews.map((p) => (
            <li key={p.label} className="flex flex-col items-center gap-1">
              {/* eslint-disable-next-line @next/next/no-img-element -- URL storage esterno */}
              <img src={p.url} alt={p.label} className={wide ? "h-24 rounded-md border border-border object-cover" : "rounded-md border border-border"} width={wide ? undefined : Math.min(Number.parseInt(p.label) || 48, 96)} />
              <span className="text-[11px] text-muted">{p.label}</span>
            </li>
          ))}
        </ul>
      ) : (
        <div className="flex h-16 items-center justify-center rounded-md border border-dashed border-border/70 text-[12px] text-muted">{t("noImage")}</div>
      )}
      <div className="flex items-center gap-3">
        <Button type="button" variant="secondary" size="sm" disabled={busy} onClick={() => input.current?.click()}>
          {busy ? t("uploading") : previews.length > 0 ? t("replace") : t("upload")}
        </Button>
        {previews.length > 0 && (
          <button type="button" disabled={busy} onClick={onRemove} className="text-sm text-muted hover:text-foreground">
            {t("remove")}
          </button>
        )}
        <input
          ref={input}
          type="file"
          accept="image/png,image/jpeg,image/webp"
          className="hidden"
          onChange={(e) => {
            const f = e.target.files?.[0];
            e.target.value = "";
            if (f) onUpload(f);
          }}
        />
      </div>
    </div>
  );
}

/** Identità e SEO del portale: icona/favicon e immagine di condivisione, titolo e descrizione per lingua, verifica
 * dei motori di ricerca, meta tag aggiuntivi, blocco dell'indicizzazione e indirizzi delle sitemap. */
export default function AdminSeoPage() {
  const { authFetch } = useAuth();
  const t = useTranslations("AdminSeo");
  const tc = useTranslations("Common");
  const notify = useToast();
  const [data, setData] = useState<SeoAdmin | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<"save" | "icon" | "social" | null>(null);
  const [siteName, setSiteName] = useState("");
  const [noindex, setNoindex] = useState(false);
  const [google, setGoogle] = useState("");
  const [bing, setBing] = useState("");
  const [yandex, setYandex] = useState("");
  const [twitter, setTwitter] = useState("");
  const [color, setColor] = useState("");
  const [extra, setExtra] = useState<{ name: string; content: string }[]>([]);
  const [locales, setLocales] = useState<Record<string, LocaleForm>>({});
  const [active, setActive] = useState<string>("it");
  const [newLocale, setNewLocale] = useState("");

  const apply = useCallback((d: SeoAdmin) => {
    setData(d);
    const b = d.branding;
    setSiteName(b.site_name ?? "");
    setNoindex(b.noindex);
    setGoogle(b.google_verification ?? "");
    setBing(b.bing_verification ?? "");
    setYandex(b.yandex_verification ?? "");
    setTwitter(b.twitter_site ?? "");
    setColor(b.theme_color ?? "");
    setExtra(b.extra_meta ?? []);
    const next: Record<string, LocaleForm> = {};
    for (const l of LOCALES) next[l] = { ...emptyLocale };
    for (const [code, v] of Object.entries(d.locales) as [string, SeoLocaleText][]) {
      next[code] = { title: v.title ?? "", description: v.description ?? "", keywords: v.keywords ?? "" };
    }
    setLocales(next);
  }, []);
  useEffect(() => {
    authFetch((tk) => api.seo.get(tk))
      .then(apply)
      .catch((err) => setError(errorMessage(err, tc("unexpectedError"))));
  }, [authFetch, apply, tc]);

  async function save(event: FormEvent) {
    event.preventDefault();
    setBusy("save");
    setError(null);
    try {
      const filled = Object.fromEntries(Object.entries(locales).filter(([, v]) => v.title.trim() || v.description.trim() || v.keywords.trim()));
      apply(
        await authFetch((tk) =>
          api.seo.save(tk, {
            site_name: siteName.trim() || undefined,
            noindex,
            google_verification: google.trim() || undefined,
            bing_verification: bing.trim() || undefined,
            yandex_verification: yandex.trim() || undefined,
            twitter_site: twitter.trim() || undefined,
            theme_color: color.trim() || undefined,
            extra_meta: extra.filter((m) => m.name.trim() || m.content.trim()),
            locales: filled,
          })
        )
      );
      notify(t("saved"));
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    } finally {
      setBusy(null);
    }
  }

  async function image(kind: "icon" | "social", fn: (token: string) => Promise<SeoAdmin>, done: string) {
    setBusy(kind);
    setError(null);
    try {
      const out = await authFetch(fn);
      // le sole immagini cambiano: i testi in modifica non si toccano
      setData(out);
      notify(done);
    } catch (err) {
      setError(errorMessage(err, tc("unexpectedError")));
    } finally {
      setBusy(null);
    }
  }

  if (!data) return error ? <Alert kind="error">{error}</Alert> : <SkeletonRows rows={6} />;

  const current = locales[active] ?? emptyLocale;
  const setCurrent = (patch: Partial<LocaleForm>) => setLocales((s) => ({ ...s, [active]: { ...(s[active] ?? emptyLocale), ...patch } }));
  const codes = Array.from(new Set([...LOCALES, ...Object.keys(locales)]));
  const iconPreviews = Object.entries(data.branding.icons)
    .sort((a, b) => Number(a[0]) - Number(b[0]))
    .map(([side, url]) => ({ label: `${side}×${side}`, url }));
  const host = new URL(SITE_URL).host;

  return (
    <form onSubmit={save} className="flex max-w-3xl flex-col gap-6">
      <div>
        <h1 className="font-serif text-2xl text-foreground">{t("title")}</h1>
        <p className="mt-2 text-sm text-muted">{t("intro")}</p>
      </div>
      {error && <Alert kind="error">{error}</Alert>}
      {noindex && <Alert kind="error">{t("noindexActive")}</Alert>}

      <Card>
        <CardTitle>{t("identityTitle")}</CardTitle>
        <div className="mt-4 flex flex-col gap-6">
          <FieldGroup className="mb-0">
            <Label htmlFor="seo-site-name" hint={t("siteNameHint")}>
              {t("siteName")}
            </Label>
            <Input id="seo-site-name" maxLength={100} placeholder="Notturni" value={siteName} onChange={(e) => setSiteName(e.target.value)} />
          </FieldGroup>
          <ImageSlot
            title={t("iconTitle")}
            hint={t("iconHint")}
            previews={iconPreviews}
            busy={busy === "icon"}
            onUpload={(f) => void image("icon", (tk) => api.seo.uploadIcon(tk, f), t("iconSaved"))}
            onRemove={() => void image("icon", (tk) => api.seo.deleteIcon(tk), t("iconRemoved"))}
          />
          <ImageSlot
            title={t("socialTitle")}
            hint={t("socialHint")}
            previews={data.branding.social_image_url ? [{ label: "1200×630", url: data.branding.social_image_url }] : []}
            busy={busy === "social"}
            wide
            onUpload={(f) => void image("social", (tk) => api.seo.uploadSocial(tk, f), t("socialSaved"))}
            onRemove={() => void image("social", (tk) => api.seo.deleteSocial(tk), t("socialRemoved"))}
          />
          <FieldGroup className="mb-0">
            <Label htmlFor="seo-color" hint={t("themeColorHint")}>
              {t("themeColor")}
            </Label>
            <div className="flex items-center gap-3">
              <Input id="seo-color" className="max-w-[160px]" placeholder="#3d6b5c" maxLength={7} value={color} onChange={(e) => setColor(e.target.value)} />
              {/^#[0-9a-fA-F]{6}$/.test(color) && <span aria-hidden="true" className="h-8 w-8 rounded-md border border-border" style={{ background: color }} />}
            </div>
          </FieldGroup>
        </div>
      </Card>

      <Card>
        <CardTitle>{t("textsTitle")}</CardTitle>
        <p className="mt-1 mb-3 text-[13px] text-muted">{t("textsIntro")}</p>
        <div className="mb-4 flex flex-wrap items-center gap-2">
          {codes.map((code) => (
            <button
              key={code}
              type="button"
              aria-pressed={active === code}
              onClick={() => setActive(code)}
              className={`rounded-full px-3 py-1 text-xs ${active === code ? "bg-foreground text-background" : "border border-border text-muted hover:text-foreground"}`}
            >
              {isLocale(code) ? LOCALE_NAMES[code] : code.toUpperCase()}
              {(locales[code]?.title || locales[code]?.description) && <span aria-hidden="true"> ·</span>}
            </button>
          ))}
          <span className="ml-2 flex items-center gap-1">
            <Input aria-label={t("addLanguage")} placeholder="de" className="w-14 py-1 text-xs" maxLength={2} value={newLocale} onChange={(e) => setNewLocale(e.target.value.toLowerCase())} />
            <Button
              type="button"
              size="sm"
              variant="secondary"
              onClick={() => {
                if (/^[a-z]{2}$/.test(newLocale)) {
                  setLocales((s) => ({ ...s, [newLocale]: s[newLocale] ?? { ...emptyLocale } }));
                  setActive(newLocale);
                  setNewLocale("");
                }
              }}
            >
              +
            </Button>
          </span>
        </div>
        <FieldGroup>
          <Label htmlFor="seo-title" hint={`${current.title.length}/${TITLE_HINT}`}>
            {t("pageTitle")}
          </Label>
          <Input id="seo-title" maxLength={120} value={current.title} onChange={(e) => setCurrent({ title: e.target.value })} />
        </FieldGroup>
        <FieldGroup>
          <Label htmlFor="seo-desc" hint={`${current.description.length}/${DESCRIPTION_HINT}`}>
            {t("description")}
          </Label>
          <TextArea id="seo-desc" rows={3} maxLength={400} className="min-h-0" value={current.description} onChange={(e) => setCurrent({ description: e.target.value })} />
        </FieldGroup>
        <FieldGroup>
          <Label htmlFor="seo-keywords" hint={t("keywordsHint")}>
            {t("keywords")}
          </Label>
          <Input id="seo-keywords" maxLength={300} value={current.keywords} onChange={(e) => setCurrent({ keywords: e.target.value })} />
        </FieldGroup>
        <div className="rounded-lg border border-border bg-background px-4 py-3" aria-label={t("snippet")}>
          <p className="text-[12px] text-muted">{host}</p>
          <p className="text-lg leading-snug text-primary">{current.title || siteName || "Notturni"}</p>
          <p className="text-[13px] text-muted">{current.description || t("noDescription")}</p>
        </div>
      </Card>

      <Card>
        <CardTitle>{t("indexingTitle")}</CardTitle>
        <div className="mt-3 rounded-xl border border-border bg-surface px-4 py-3">
          <Toggle checked={noindex} onChange={setNoindex} label={t("noindex")} />
          <p className="mt-1 text-[13px] text-muted">{t("noindexHint")}</p>
        </div>
        <div className="mt-4 grid gap-x-4 sm:grid-cols-2">
          <FieldGroup>
            <Label htmlFor="seo-google" hint={t("verificationHint")}>
              Google Search Console
            </Label>
            <Input id="seo-google" autoComplete="off" value={google} onChange={(e) => setGoogle(e.target.value)} />
          </FieldGroup>
          <FieldGroup>
            <Label htmlFor="seo-bing" hint={t("verificationHint")}>
              Bing Webmaster
            </Label>
            <Input id="seo-bing" autoComplete="off" value={bing} onChange={(e) => setBing(e.target.value)} />
          </FieldGroup>
          <FieldGroup>
            <Label htmlFor="seo-yandex" hint={t("verificationHint")}>
              Yandex Webmaster
            </Label>
            <Input id="seo-yandex" autoComplete="off" value={yandex} onChange={(e) => setYandex(e.target.value)} />
          </FieldGroup>
          <FieldGroup>
            <Label htmlFor="seo-twitter" hint={t("twitterHint")}>
              {t("twitter")}
            </Label>
            <Input id="seo-twitter" placeholder="@notturni" maxLength={50} value={twitter} onChange={(e) => setTwitter(e.target.value)} />
          </FieldGroup>
        </div>
        <div>
          <h3 className="text-sm font-semibold text-foreground">{t("extraTitle")}</h3>
          <p className="mb-2 text-[12px] text-muted">{t("extraHint")}</p>
          <ul className="flex flex-col gap-2">
            {extra.map((m, i) => (
              <li key={i} className="flex gap-2">
                <Input aria-label={t("extraName")} placeholder={t("extraName")} className="max-w-[220px]" value={m.name} onChange={(e) => setExtra((l) => l.map((x, j) => (j === i ? { ...x, name: e.target.value } : x)))} />
                <Input aria-label={t("extraContent")} placeholder={t("extraContent")} value={m.content} onChange={(e) => setExtra((l) => l.map((x, j) => (j === i ? { ...x, content: e.target.value } : x)))} />
                <button type="button" aria-label={tc("remove")} className="px-2 text-muted hover:text-foreground" onClick={() => setExtra((l) => l.filter((_, j) => j !== i))}>
                  ×
                </button>
              </li>
            ))}
          </ul>
          {extra.length < 10 && (
            <Button type="button" size="sm" variant="secondary" className="mt-2" onClick={() => setExtra((l) => [...l, { name: "", content: "" }])}>
              {t("extraAdd")}
            </Button>
          )}
        </div>
      </Card>

      <Card>
        <CardTitle>{t("sitemapTitle")}</CardTitle>
        <p className="mt-1 mb-3 text-[13px] text-muted">{t("sitemapIntro")}</p>
        <dl className="grid gap-x-4 gap-y-2 text-[13px] sm:grid-cols-[170px_minmax(0,1fr)]">
          <dt className="text-muted">{t("sitemapGlobal")}</dt>
          <dd className="break-all font-mono text-xs">
            <a href={`${SITE_URL}/sitemap.xml`} target="_blank" rel="noreferrer" className="text-primary no-underline hover:underline">{`${SITE_URL}/sitemap.xml`}</a>
          </dd>
          <dt className="text-muted">{t("sitemapSpace")}</dt>
          <dd className="break-all font-mono text-xs">{`${SITE_URL}/{slug}/sitemap.xml`}</dd>
          <dt className="text-muted">{t("sitemapSubdomain")}</dt>
          <dd className="break-all font-mono text-xs">{`https://{slug}.${host}/sitemap.xml`}</dd>
          <dt className="text-muted">robots.txt</dt>
          <dd className="break-all font-mono text-xs">
            <a href={`${SITE_URL}/robots.txt`} target="_blank" rel="noreferrer" className="text-primary no-underline hover:underline">{`${SITE_URL}/robots.txt`}</a>
          </dd>
        </dl>
      </Card>

      <div className="sticky bottom-4 flex items-center gap-3 rounded-xl border border-border bg-surface px-4 py-3">
        <Button type="submit" disabled={busy !== null}>
          {busy === "save" ? tc("saving") : tc("save")}
        </Button>
        <span className="text-[12px] text-muted">{t("saveHint")}</span>
      </div>
    </form>
  );
}
