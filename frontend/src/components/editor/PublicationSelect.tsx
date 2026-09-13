"use client";

import { useTranslations } from "next-intl";
import { useEffect, useState } from "react";

import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import type { Publication } from "@/lib/types";

/** Selettore della pubblicazione del post (B9): al più una; nessuna
 * pubblicazione creata sul blog → non mostra nulla. */
export function PublicationSelect({ blogSlug, value, onChange }: { blogSlug: string; value: string | null; onChange: (id: string | null) => void }) {
  const { accessToken } = useAuth();
  const t = useTranslations("PublicationsTab");
  const [publications, setPublications] = useState<Publication[]>([]);

  useEffect(() => {
    api.blogs.listPublications(blogSlug, accessToken).then(setPublications).catch(() => undefined);
  }, [blogSlug, accessToken]);

  if (publications.length === 0) return null;

  return (
    <label className="flex flex-col gap-1">
      <span className="text-[11px] font-medium uppercase tracking-wide text-muted">{t("publication")}</span>
      <select
        value={value ?? ""}
        onChange={(e) => onChange(e.target.value || null)}
        className="rounded-lg border border-border bg-background px-3 py-2 text-sm font-medium text-foreground transition hover:border-primary/50 focus:outline-none focus:ring-2 focus:ring-primary/40"
      >
        <option value="">{t("noPublication")}</option>
        {publications.map((p) => (
          <option key={p.id} value={p.id}>
            {p.title}
          </option>
        ))}
      </select>
    </label>
  );
}
