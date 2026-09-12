"use client";

import { useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle, SectionLabel } from "@/components/ui/Card";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { SANS_SERIF_FONTS, SERIF_FONTS, type BlogConfig } from "@/lib/types";

const FONT_OPTIONS: Record<string, string[]> = {
  heading_font: SERIF_FONTS,
  body_font: SANS_SERIF_FONTS,
};

import { errorMessage } from "./shared";

export function AppearanceTab({ blogSlug, canEdit }: { blogSlug: string; canEdit: boolean }) {
  const { accessToken, authFetch } = useAuth();
  const [config, setConfig] = useState<BlogConfig | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    api.blogs
      .getConfig(blogSlug, accessToken)
      .then(setConfig)
      .catch((err) => setError(errorMessage(err)));
  }, [blogSlug, accessToken]);

  function updatePaletteColor(key: string, value: string) {
    setConfig((prev) => ({ ...prev, palette: { ...prev?.palette, [key]: value } }));
    setSaved(false);
  }

  function updateTypography(key: string, value: string) {
    setConfig((prev) => ({ ...prev, typography: { ...prev?.typography, [key]: value } }));
    setSaved(false);
  }

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

  if (!config) return <p className="text-sm text-muted">Caricamento…</p>;

  const paletteEntries = Object.entries(config.palette ?? {});
  const typographyEntries = Object.entries(config.typography ?? {});

  return (
    <div className="flex flex-col gap-6">
      <Card className="flex flex-col gap-3">
        <div className="flex items-baseline justify-between">
          <CardTitle>Palette</CardTitle>
          <span className="text-[13px] text-muted">{paletteEntries.length} di 5 colori</span>
        </div>
        <div className="grid grid-cols-2 gap-2.5 sm:grid-cols-3 lg:grid-cols-5">
          {paletteEntries.map(([key, value]) => (
            <label
              key={key}
              htmlFor={`color-${key}`}
              className="flex cursor-pointer flex-col gap-2 rounded-lg border border-border bg-surface p-2.5"
            >
              <span
                className="block h-14 rounded-md border border-border"
                style={{ background: value }}
              />
              <span className="text-[13px] font-medium text-foreground">{key}</span>
              <span className="font-mono text-xs text-muted">{value}</span>
              <input
                id={`color-${key}`}
                type="color"
                value={value}
                onChange={(e) => updatePaletteColor(key, e.target.value)}
                className="sr-only"
              />
            </label>
          ))}
        </div>
      </Card>

      <Card className="flex flex-col gap-3">
        <div className="flex items-baseline justify-between">
          <CardTitle>Tipografia</CardTitle>
          <span className="text-[13px] text-muted">titoli serif · corpo sans-serif</span>
        </div>
        <div className="flex flex-wrap gap-4">
          {typographyEntries.map(([key, value]) => {
            const options = FONT_OPTIONS[key];
            return (
              <FieldGroup key={key} className="min-w-[220px] flex-1">
                <Label htmlFor={`font-${key}`}>{key}</Label>
                {options ? (
                  <select
                    id={`font-${key}`}
                    value={value}
                    onChange={(e) => updateTypography(key, e.target.value)}
                    className="w-full rounded-lg border border-border bg-surface px-3 py-2.5 text-sm text-foreground focus:outline-none focus:border-primary focus:ring-[3px] focus:ring-primary/20"
                  >
                    {options.map((font) => (
                      <option key={font} value={font}>
                        {font}
                      </option>
                    ))}
                  </select>
                ) : (
                  <Input
                    id={`font-${key}`}
                    value={value}
                    onChange={(e) => updateTypography(key, e.target.value)}
                  />
                )}
              </FieldGroup>
            );
          })}
        </div>
      </Card>

      <Card className="flex flex-col gap-3">
        <CardTitle>Layout</CardTitle>
        <FieldGroup className="max-w-xs">
          <SectionLabel>Presentazione dei post</SectionLabel>
          <select
            value={config.layout ?? "standard"}
            onChange={(e) => {
              setConfig((prev) => ({ ...prev, layout: e.target.value }));
              setSaved(false);
            }}
            className="mt-1.5 w-full rounded-lg border border-border bg-surface px-3 py-2.5 text-sm text-foreground focus:outline-none focus:border-primary focus:ring-[3px] focus:ring-primary/20"
          >
            <option value="standard">Standard</option>
            <option value="magazine">Magazine</option>
            <option value="minimal">Minimale</option>
          </select>
        </FieldGroup>
      </Card>

      {error && <Alert kind="error">{error}</Alert>}
      {saved && <Alert kind="success">Salvato.</Alert>}
      {canEdit && (
        <div>
          <Button onClick={handleSave} disabled={saving}>
            {saving ? "Salvataggio…" : "Salva aspetto"}
          </Button>
        </div>
      )}
    </div>
  );
}
