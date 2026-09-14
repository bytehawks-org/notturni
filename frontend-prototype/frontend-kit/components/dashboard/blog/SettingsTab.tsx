"use client";
import { useTranslations } from "next-intl";
import { useState } from "react";
import { Button } from "@/components/ui/Button";
import { Input, TextArea, Label, FieldGroup } from "@/components/ui/Field";
import { Toggle } from "@/components/ui/Controls";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import type { Visibility } from "@/components/blog/VisibilityBand";

export interface BlogSettings {
  title: string; slug: string; subtitle: string; description: string; defaultAuthorName: string;
  visibility: Visibility; indexing: boolean; languages: string[]; primaryLanguage: string;
  features: { mentions: boolean; fragments: boolean; bibliography: boolean; pages: boolean; publications: boolean; brainMap: boolean };
  customDomain?: string;
}

const SECTIONS = ["identity", "visibility", "languages", "features", "newsletter", "domain", "danger-zone"];
const VIS: { v: Visibility; band: string }[] = [{ v: "public", band: "var(--vis-public)" }, { v: "members", band: "var(--vis-members)" }, { v: "private", band: "var(--vis-private)" }];
const FEATURES: (keyof BlogSettings["features"])[] = ["mentions", "fragments", "bibliography", "pages", "publications", "brainMap"];

function Section({ id, title, children }: { id: string; title: string; children: React.ReactNode }) {
  return <section id={id} className="flex flex-col gap-3.5"><h2 className="font-serif text-lg">{title}</h2>{children}</section>;
}

export function SettingsForm({ initial, onSave, onDanger }: { initial: BlogSettings; onSave: (s: BlogSettings) => Promise<void>; onDanger: (a: "transfer" | "pause" | "export" | "delete") => void }) {
  const t = useTranslations("Settings");
  const [s, setS] = useState(initial);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const up = <K extends keyof BlogSettings>(k: K, v: BlogSettings[K]) => setS((p) => ({ ...p, [k]: v }));
  return (
    <div className="grid gap-8 text-sm lg:grid-cols-[180px_minmax(0,1fr)] lg:gap-10">
      <nav className="hidden flex-col gap-0.5 self-start text-muted lg:sticky lg:top-6 lg:flex">
        {SECTIONS.map((x) => <a key={x} href={`#${x}`} className={`rounded-md px-2.5 py-[7px] no-underline hover:text-foreground ${x === "danger-zone" ? "text-danger" : ""}`}>{t(`sections.${x}`)}</a>)}
      </nav>
      <div className="flex max-w-[760px] flex-col gap-9">
        <Section id="identity" title={t("sections.identity")}>
          <div className="grid gap-3.5 md:grid-cols-2">
            <FieldGroup><Label>{t("title")}</Label><Input value={s.title} onChange={(e) => up("title", e.target.value)} /></FieldGroup>
            <FieldGroup><Label hint={t("slugHint")}>Slug</Label><div className="flex items-center rounded-lg border border-border bg-surface px-3 font-mono text-[13px]"><input value={s.slug} onChange={(e) => up("slug", e.target.value)} className="w-full bg-transparent py-2.5 outline-none" /><span className="text-muted">.notturni.eu</span></div></FieldGroup>
          </div>
          <FieldGroup><Label hint={`${s.subtitle.length} / 64`}>{t("subtitle")}</Label><Input maxLength={64} value={s.subtitle} onChange={(e) => up("subtitle", e.target.value)} /></FieldGroup>
          <FieldGroup><Label hint={`${s.description.length} / 256`}>{t("description")}</Label><TextArea maxLength={256} className="min-h-16" value={s.description} onChange={(e) => up("description", e.target.value)} /></FieldGroup>
          <FieldGroup><Label>{t("defaultAuthor")}</Label><Input value={s.defaultAuthorName} onChange={(e) => up("defaultAuthorName", e.target.value)} /></FieldGroup>
        </Section>
        <Section id="visibility" title={t("sections.visibility")}>
          <div role="radiogroup" className="grid gap-2.5 md:grid-cols-3">
            {VIS.map((o) => (
              <button key={o.v} role="radio" aria-checked={s.visibility === o.v} onClick={() => up("visibility", o.v)} className={`relative flex flex-col gap-1 overflow-hidden rounded-xl border bg-surface px-4 py-3.5 text-left ${s.visibility === o.v ? "border-primary" : "border-border"}`}>
                <span aria-hidden="true" className="absolute inset-y-0 right-0 w-1.5" style={{ background: o.band }} />
                <span className="font-semibold">{t(`vis.${o.v}`)}</span><span className="pr-2 text-xs leading-snug text-muted">{t(`vis.${o.v}Sub`)}</span>
              </button>
            ))}
          </div>
          <div className="flex items-center justify-between gap-4"><div className="flex flex-col"><span>{t("indexing")}</span><span className="text-xs text-muted">{t("indexingSub")}</span></div><Toggle label="" checked={s.indexing} onChange={(v) => up("indexing", v)} /></div>
        </Section>
        <Section id="languages" title={t("sections.languages")}>
          <div className="flex flex-wrap items-center gap-2">
            <span className="rounded-full bg-foreground px-[11px] py-[5px] text-[13px] text-background">{t("primary", { lang: s.primaryLanguage })}</span>
            {s.languages.filter((l) => l !== s.primaryLanguage).map((l) => <button key={l} onClick={() => up("languages", s.languages.filter((x) => x !== l))} className="rounded-full border border-border px-[11px] py-[5px] text-[13px]">{l} ×</button>)}
            <button className="rounded-full border border-dashed border-border px-[11px] py-[5px] text-[13px] text-muted">{t("addLanguage")}</button>
          </div>
          <span className="text-xs text-muted">{t("languagesNote")}</span>
        </Section>
        <Section id="features" title={t("sections.features")}>
          {FEATURES.map((k) => <div key={k} className="flex items-center justify-between gap-4 border-b border-border py-2.5"><div className="flex flex-col"><span>{t(`features.${k}`)}</span><span className="text-xs text-muted">{t(`features.${k}Sub`)}</span></div><Toggle label="" checked={s.features[k]} onChange={(v) => up("features", { ...s.features, [k]: v })} /></div>)}
        </Section>
        <Section id="domain" title={t("sections.domain")}>
          <div className="grid items-end gap-2.5 md:grid-cols-[minmax(0,1fr)_auto]"><FieldGroup><Label>{t("customDomain")}</Label><Input disabled className="font-mono text-[13px]" placeholder="quaderno.example.eu" value={s.customDomain ?? ""} /></FieldGroup><Button variant="secondary" disabled>{t("verifyDns")}</Button></div>
          <span className="text-xs text-muted">{t("domainNote", { host: `${s.slug}.notturni.eu` })}</span>
        </Section>
        <section id="danger-zone" className="flex flex-col gap-3 rounded-xl border border-danger bg-danger/4 p-5">
          <h2 className="font-serif text-lg text-danger">{t("dangerZone")}</h2>
          {(["transfer", "pause", "export"] as const).map((a) => (
            <div key={a} className="flex items-center justify-between gap-5 border-b border-border py-2.5"><div className="flex flex-col"><span className="font-medium">{t(`danger.${a}`)}</span><span className="text-xs text-muted">{t(`danger.${a}Sub`)}</span></div><Button variant="secondary" size="sm" onClick={() => onDanger(a)}>{t(`danger.${a}Action`)}</Button></div>
          ))}
          <div className="flex items-center justify-between gap-5 py-2.5"><div className="flex flex-col"><span className="font-medium">{t("danger.delete")}</span><span className="text-xs text-muted">{t("danger.deleteSub")}</span></div><Button variant="danger" size="sm" onClick={() => setConfirmDelete(true)}>{t("danger.deleteAction")}</Button></div>
        </section>
        <div className="sticky bottom-4 flex justify-end lg:static"><Button onClick={() => onSave(s)}>{t("save")}</Button></div>
      </div>
      <ConfirmDialog open={confirmDelete} title={t("confirmTitle", { title: s.title })} body={t("confirmBody")} confirmText={s.slug} confirmLabel={t("confirmLabel")} onCancel={() => setConfirmDelete(false)} onConfirm={() => { setConfirmDelete(false); onDanger("delete"); }} />
    </div>
  );
}
