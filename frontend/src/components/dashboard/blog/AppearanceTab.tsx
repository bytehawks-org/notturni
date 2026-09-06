"use client";

import { useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { type BlogConfig } from "@/lib/types";

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
    <Card>
      <CardTitle>Palette (massimo 5 colori)</CardTitle>
      <div className="mb-6 flex flex-wrap gap-4">
        {paletteEntries.map(([key, value]) => (
          <div key={key}>
            <Label htmlFor={`color-${key}`}>{key}</Label>
            <div className="flex items-center gap-2">
              <input
                id={`color-${key}`}
                type="color"
                value={value}
                onChange={(e) => updatePaletteColor(key, e.target.value)}
                className="h-9 w-9 cursor-pointer rounded border border-border"
              />
              <span className="text-xs text-muted">{value}</span>
            </div>
          </div>
        ))}
      </div>

      <CardTitle>Tipografia (massimo 3 font)</CardTitle>
      <div className="mb-6 flex flex-wrap gap-4">
        {typographyEntries.map(([key, value]) => (
          <FieldGroup key={key}>
            <Label htmlFor={`font-${key}`}>{key}</Label>
            <Input id={`font-${key}`} value={value} onChange={(e) => updateTypography(key, e.target.value)} />
          </FieldGroup>
        ))}
      </div>

      <CardTitle>Layout</CardTitle>
      <FieldGroup>
        <select
          value={config.layout ?? "standard"}
          onChange={(e) => {
            setConfig((prev) => ({ ...prev, layout: e.target.value }));
            setSaved(false);
          }}
          className="w-full max-w-xs rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground"
        >
          <option value="standard">Standard</option>
          <option value="magazine">Magazine</option>
          <option value="minimal">Minimale</option>
        </select>
      </FieldGroup>

      {error && (
        <div className="mb-4">
          <Alert kind="error">{error}</Alert>
        </div>
      )}
      {saved && (
        <div className="mb-4">
          <Alert kind="success">Salvato.</Alert>
        </div>
      )}
      {canEdit && (
        <Button onClick={handleSave} disabled={saving}>
          {saving ? "Salvataggio…" : "Salva aspetto"}
        </Button>
      )}
    </Card>
  );
}
