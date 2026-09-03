"use client";

import { useEffect, useState } from "react";

import { api } from "@/lib/api";
import type { Category } from "@/lib/types";

interface CategorySelectProps {
  blogSlug: string;
  value: string | null;
  onChange: (categoryId: string | null) => void;
}

/** Selettore della categoria del post (tassonomia del blog, al più una per
 * post — a differenza dei tag). Nessuna categoria creata sul blog: non
 * mostra nulla, non è un campo obbligatorio. */
export function CategorySelect({ blogSlug, value, onChange }: CategorySelectProps) {
  const [categories, setCategories] = useState<Category[]>([]);

  useEffect(() => {
    api.blogs.listCategories(blogSlug).then(setCategories).catch(() => undefined);
  }, [blogSlug]);

  if (categories.length === 0) return null;

  return (
    <label className="flex flex-col gap-1">
      <span className="text-[11px] font-medium uppercase tracking-wide text-muted">Categoria</span>
      <select
        value={value ?? ""}
        onChange={(e) => onChange(e.target.value || null)}
        className="rounded-lg border border-border bg-background px-3 py-2 text-sm font-medium text-foreground transition hover:border-primary/50 focus:outline-none focus:ring-2 focus:ring-primary/40"
      >
        <option value="">Nessuna categoria</option>
        {categories.map((c) => (
          <option key={c.id} value={c.id}>
            {c.name}
          </option>
        ))}
      </select>
    </label>
  );
}
