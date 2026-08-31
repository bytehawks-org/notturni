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
    <select
      value={value ?? ""}
      onChange={(e) => onChange(e.target.value || null)}
      className="rounded-md border border-border bg-background px-3 py-1.5 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-primary/40"
    >
      <option value="">Nessuna categoria</option>
      {categories.map((c) => (
        <option key={c.id} value={c.id}>
          {c.name}
        </option>
      ))}
    </select>
  );
}
