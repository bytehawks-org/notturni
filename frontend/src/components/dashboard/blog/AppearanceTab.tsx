"use client";

import { useTranslations } from "next-intl";
import { useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle, SectionLabel } from "@/components/ui/Card";
import { SegmentedControl } from "@/components/ui/Controls";
import { FieldGroup, Label, TextArea } from "@/components/ui/Field";
import { SkeletonRows } from "@/components/ui/States";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { contrastRatio, deriveDarkPalette } from "@/lib/contrast";
import { MONOSPACE_FONTS, SANS_SERIF_FONTS, SERIF_FONTS, type BlogConfig } from "@/lib/types";

import { errorMessage } from "./shared";

const FONT_OPTIONS: Record<string, string[]> = {
  heading_font: SERIF_FONTS,
  body_font: SANS_SERIF_FONTS,
  monospace_font: MONOSPACE_FONTS,
};

/** Preset di palette calme (saturazione < 90%, vincolo del backend). */
const PRESETS: { id: string; palette: Record<string, string> }[] = [
  { id: "notturni", palette: { background: "#faf8f4", foreground: "#232220", primary: "#3d6b5e", muted: "#857e74", border: "#e5dfd5" } },
  { id: "carta", palette: { background: "#f6f1e7", foreground: "#2d2a25", primary: "#8a5a2b", muted: "#8c8273", border: "#e3d9c8" } },
  { id: "ardesia", palette: { background: "#f3f4f6", foreground: "#1f2429", primary: "#3d5a80", muted: "#7b838d", border: "#dfe3e8" } },
  { id: "lavanda", palette: { background: "#f8f6fb", foreground: "#262330", primary: "#6b5b95", muted: "#8a8497", border: "#e6e1ee" } },
];

const BODY_SIZES = ["17", "18", "19"] as const;
const MEASURES = ["narrow", "normal"] as const;
const LAYOUTS = ["standard", "magazine", "minimal"] as const;

/** Tab Aspetto (mockup 2e): palette (5 colori) con preset e verifica AA,
 * tipografia dagli elenchi curati, corpo/misura/layout, anteprima live.
 * "Genera variante scura" salva `palette_dark` (derivata da lib/contrast.ts). */
export function AppearanceTab({ blogSlug, canEdit }: { blogSlug: string; canEdit: boolean }) {
  const { accessToken, authFetch } = useAuth();
  const t = useTranslations("Appearance");
  const tc = useTranslations("Common");
  const [config, setConfig] = useState<BlogConfig | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [previewDark, setPreviewDark] = useState(false);

  useEffect(() => {
    api.blogs
      .getConfig(blogSlug, accessToken)
      .then(setConfig)
      .catch((err) => setError(errorMessage(err)));
  }, [blogSlug, accessToken]);

  function patch(fn: (prev: BlogConfig) => BlogConfig) {
    setConfig((prev) => fn(prev ?? {}));
    setSaved(false);
  }
  const updatePaletteColor = (key: string, value: string) => patch((p) => ({ ...p, palette: { ...p.palette, [key]: value } }));
  const updateTypography = (key: string, value: string) => patch((p) => ({ ...p, typography: { ...p.typography, [key]: value } }));
  const updateFooter = (key: "column1" | "column2", value: string) =>
    patch((p) => ({ ...p, footer: { ...p.footer, [key]: value } }));

  async function handleSave() {
    if (!config) return;
    setSaving(true);
    setError(null);
    try {
      const updated = await authFetch((token) => api.blogs.updateConfig(token, blogSlug, config));
      setConfig(updated);
      setSaved(true);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSaving(false);
    }
  }

  if (!config) return error ? <Alert kind="error">{error}</Alert> : <SkeletonRows rows={4} />;

  const palette = config.palette ?? {};
  const darkPalette = config.palette_dark;
  const typography = config.typography ?? {};
  const footer = config.footer ?? {};
  const paletteEntries = Object.entries(palette);
  const shown = previewDark && darkPalette ? darkPalette : palette;
  const bg = shown.background ?? (previewDark ? "#18191b" : "#faf8f4");
  const fg = shown.foreground ?? (previewDark ? "#ebe6de" : "#232220");
  const primary = shown.primary ?? (previewDark ? "#83b8a5" : "#3d6b5e");
  const muted = shown.muted ?? (previewDark ? "#968e82" : "#857e74");
  const textRatio = contrastRatio(fg, bg);
  const primaryRatio = contrastRatio(primary, bg);
  const mutedRatio = contrastRatio(muted, bg);
  const aaOk = (textRatio ?? 0) >= 4.5 && (primaryRatio ?? 0) >= 3 && (mutedRatio ?? 0) >= 3;
  const bodySize = (typography.body_size as string) ?? "18";
  const measure = (typography.measure as string) ?? "normal";

  const previewStyle = {
    background: bg,
    color: fg,
    borderColor: shown.border ?? (previewDark ? "#2f2e2b" : "#e5dfd5"),
    fontFamily: `"${typography.body_font ?? "Source Sans 3"}", system-ui, sans-serif`,
    fontSize: `${bodySize}px`,
  } as const;

  return (
    <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_360px]">
      <div className="flex flex-col gap-6">
        <Card className="flex flex-col gap-4">
          <div className="flex flex-wrap items-baseline justify-between gap-2">
            <CardTitle>{t("palette")}</CardTitle>
            <span className="text-[13px] text-muted">{t("paletteHint", { count: paletteEntries.length })}</span>
          </div>
          <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-3 lg:grid-cols-5">
            {paletteEntries.map(([key, value]) => (
              <label key={key} htmlFor={`color-${key}`} className="flex cursor-pointer flex-col gap-2 rounded-lg border border-border bg-surface p-2.5">
                <span className="block h-14 rounded-md border border-border" style={{ background: value }} />
                <span className="text-[13px] font-medium text-foreground">{t.has(`color.${key}`) ? t(`color.${key}`) : key}</span>
                <span className="font-mono text-xs text-muted">{value}</span>
                <input
                  id={`color-${key}`}
                  type="color"
                  value={value}
                  disabled={!canEdit}
                  onChange={(e) => updatePaletteColor(key, e.target.value)}
                  className="sr-only"
                />
              </label>
            ))}
          </div>
          <div className="flex flex-wrap items-center gap-3 text-[13px]">
            <span className="text-muted">{t("presets")}</span>
            {PRESETS.map((preset) => (
              <button
                key={preset.id}
                type="button"
                disabled={!canEdit}
                onClick={() => patch((p) => ({ ...p, palette: { ...preset.palette } }))}
                className="flex items-center gap-1.5 rounded-full border border-border px-2.5 py-1 text-muted hover:text-foreground disabled:opacity-50"
              >
                <span className="flex overflow-hidden rounded-full border border-border">
                  {Object.values(preset.palette).map((c) => (
                    <span key={c} className="h-3 w-3" style={{ background: c }} />
                  ))}
                </span>
                {t(`preset.${preset.id}`)}
              </button>
            ))}
            <button
              type="button"
              disabled={!canEdit}
              onClick={() => patch((p) => ({ ...p, palette_dark: deriveDarkPalette(p.palette ?? {}) }))}
              className="rounded-full border border-border px-2.5 py-1 text-muted hover:text-foreground disabled:opacity-50"
            >
              {darkPalette ? t("regenerateDark") : t("generateDark")}
            </button>
            {darkPalette && canEdit && (
              <button
                type="button"
                onClick={() => patch((p) => ({ ...p, palette_dark: undefined }))}
                className="text-muted hover:text-foreground"
              >
                {t("removeDark")}
              </button>
            )}
            <span className={`ml-auto rounded-full px-2 py-0.5 text-xs font-semibold ${aaOk ? "bg-ok/15 text-ok" : "bg-danger/12 text-danger"}`}>
              {t("contrast")} · AA {aaOk ? "✓" : "✗"}
              {textRatio && ` · ${textRatio.toFixed(1)}:1`}
            </span>
          </div>
        </Card>

        <Card className="flex flex-col gap-4">
          <div className="flex items-baseline justify-between">
            <CardTitle>{t("typography")}</CardTitle>
            <span className="text-[13px] text-muted">{t("typographyHint")}</span>
          </div>
          <div className="grid gap-4 sm:grid-cols-2">
            {(["heading_font", "body_font", "monospace_font"] as const).map((key) => (
              <FieldGroup key={key} className="mb-0">
                <Label htmlFor={`font-${key}`}>{t(key)}</Label>
                <select
                  id={`font-${key}`}
                  value={(typography[key] as string) ?? FONT_OPTIONS[key][0]}
                  disabled={!canEdit}
                  onChange={(e) => updateTypography(key, e.target.value)}
                  className="w-full rounded-lg border border-border bg-surface px-3 py-2.5 text-sm text-foreground focus:outline-none focus:border-primary focus:ring-[3px] focus:ring-primary/20"
                >
                  {FONT_OPTIONS[key].map((font) => (
                    <option key={font} value={font}>
                      {font}
                    </option>
                  ))}
                </select>
              </FieldGroup>
            ))}
            <div className="flex flex-col gap-1.5">
              <SectionLabel>{t("bodySize")}</SectionLabel>
              <SegmentedControl
                value={bodySize}
                options={BODY_SIZES.map((s) => ({ value: s, label: s }))}
                onChange={(v) => canEdit && updateTypography("body_size", v)}
              />
            </div>
            <div className="flex flex-col gap-1.5">
              <SectionLabel>{t("measureLabel")}</SectionLabel>
              <SegmentedControl
                value={measure}
                options={MEASURES.map((m) => ({ value: m, label: t(`measure.${m}`) }))}
                onChange={(v) => canEdit && updateTypography("measure", v)}
              />
            </div>
          </div>
        </Card>

        <Card className="flex flex-col gap-3">
          <CardTitle>{t("layoutLabel")}</CardTitle>
          <div className="max-w-sm">
            <SegmentedControl
              value={(config.layout as (typeof LAYOUTS)[number]) ?? "standard"}
              options={LAYOUTS.map((l) => ({ value: l, label: t(`layout.${l}`) }))}
              onChange={(v) => canEdit && patch((p) => ({ ...p, layout: v }))}
            />
          </div>
        </Card>

        <Card className="flex flex-col gap-4">
          <div className="flex flex-col gap-1">
            <CardTitle>{t("footerOverride")}</CardTitle>
            <span className="text-[13px] text-muted">{t("footerOverrideHint")}</span>
          </div>
          <div className="grid gap-4 sm:grid-cols-2">
            <FieldGroup className="mb-0">
              <Label htmlFor="footer-override-col1">{t("footerOverrideColumn1")}</Label>
              <TextArea
                id="footer-override-col1"
                rows={4}
                maxLength={5000}
                disabled={!canEdit}
                value={(footer.column1 as string) ?? ""}
                onChange={(e) => updateFooter("column1", e.target.value)}
                placeholder={t("footerOverridePlaceholder")}
              />
            </FieldGroup>
            <FieldGroup className="mb-0">
              <Label htmlFor="footer-override-col2">{t("footerOverrideColumn2")}</Label>
              <TextArea
                id="footer-override-col2"
                rows={4}
                maxLength={5000}
                disabled={!canEdit}
                value={(footer.column2 as string) ?? ""}
                onChange={(e) => updateFooter("column2", e.target.value)}
                placeholder={t("footerOverridePlaceholder")}
              />
            </FieldGroup>
          </div>
        </Card>

        {error && <Alert kind="error">{error}</Alert>}
        {saved && <Alert kind="success">{t("saved")}</Alert>}
        {canEdit && (
          <div>
            <Button onClick={handleSave} disabled={saving}>
              {saving ? tc("saving") : t("save")}
            </Button>
          </div>
        )}
      </div>

      <aside className="flex flex-col gap-2">
        <div className="flex items-center justify-between">
          <SectionLabel>{t("preview")}</SectionLabel>
          {darkPalette && (
            <button type="button" onClick={() => setPreviewDark((v) => !v)} className="text-xs text-muted hover:text-foreground">
              {previewDark ? t("previewLight") : t("previewDark")}
            </button>
          )}
        </div>
        <div className="rounded-xl border p-6 leading-[1.6] shadow-soft" style={previewStyle}>
          <div className="mb-4 flex items-center gap-4 border-b pb-3 text-[13px]" style={{ borderColor: previewStyle.borderColor }}>
            <span className="font-semibold" style={{ fontFamily: `"${typography.heading_font ?? "Lora"}", serif` }}>
              {t("previewBlog")}
            </span>
            <span style={{ color: muted }}>{t("previewNav")}</span>
          </div>
          <span className="text-xs uppercase tracking-[.06em]" style={{ color: muted }}>
            {t("previewCategory")}
          </span>
          <h3 className="mt-1 text-[1.6em] font-medium leading-tight" style={{ fontFamily: `"${typography.heading_font ?? "Lora"}", serif` }}>
            {t("previewTitle")}
          </h3>
          <p className={`mt-3 ${measure === "narrow" ? "max-w-[38ch]" : "max-w-[60ch]"}`}>
            {t("previewBody")}{" "}
            <a href="#" onClick={(e) => e.preventDefault()} style={{ color: primary }}>
              {t("previewLink")}
            </a>
            <sup className="ml-0.5 text-[.7em]" style={{ color: primary }}>
              1
            </sup>
          </p>
          <p className="mt-4 text-[.85em]" style={{ color: muted }}>
            1. {t("previewNote")}
          </p>
          <code
            className="mt-3 block rounded px-2 py-1 text-[.8em]"
            style={{ fontFamily: `"${typography.monospace_font ?? "JetBrains Mono"}", monospace`, background: "color-mix(in srgb, currentColor 8%, transparent)" }}
          >
            {t("previewCode")}
          </code>
        </div>
        <span className="text-xs text-muted">{darkPalette ? t("previewHintDark") : t("previewHint")}</span>
      </aside>
    </div>
  );
}
