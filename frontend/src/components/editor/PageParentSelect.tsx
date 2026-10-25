"use client";

import { useTranslations } from "next-intl";
import { useEffect, useState } from "react";

import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { subtreeIds } from "@/lib/outline";
import type { OutlineNode } from "@/lib/types";

/** Pagina padre di un post negli Spazi wiki/docs. Non offre il post stesso né le sue sottopagine
 * (il backend rifiuterebbe il ciclo). */
export function PageParentSelect({
  blogSlug,
  postId,
  value,
  onChange,
}: {
  blogSlug: string;
  /** Assente per un post ancora da creare. */
  postId?: string;
  value: string | null;
  onChange: (id: string | null) => void;
}) {
  const { accessToken } = useAuth();
  const t = useTranslations("PageParentSelect");
  const [nodes, setNodes] = useState<OutlineNode[]>([]);

  useEffect(() => {
    api.wiki.outline(accessToken, blogSlug).then(setNodes).catch(() => undefined);
  }, [blogSlug, accessToken]);

  const own = postId ? nodes.findIndex((n) => n.id === postId) : -1;
  const excluded = own >= 0 ? subtreeIds(nodes, own) : new Set<string>();

  return (
    <label className="flex flex-col gap-1">
      <span className="text-[11px] font-medium uppercase tracking-wide text-muted">{t("label")}</span>
      <select
        value={value ?? ""}
        onChange={(e) => onChange(e.target.value || null)}
        className="rounded-lg border border-border bg-background px-3 py-2 text-sm font-medium text-foreground transition hover:border-primary/50 focus:outline-none focus:ring-2 focus:ring-primary/40"
      >
        <option value="">{t("root")}</option>
        {nodes
          .filter((n) => !excluded.has(n.id))
          .map((n) => (
            <option key={n.id} value={n.id}>
              {"— ".repeat(n.depth)}
              {n.title}
            </option>
          ))}
      </select>
    </label>
  );
}
