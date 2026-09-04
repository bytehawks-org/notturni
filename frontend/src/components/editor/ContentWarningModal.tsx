"use client";

import { useState } from "react";

import { Button } from "@/components/ui/Button";
import {
  SENSITIVITY_CATEGORY_GROUPS,
  SENSITIVITY_CATEGORY_LABELS,
  type SensitivityCategory,
} from "@/lib/content-media";

interface ContentWarningModalProps {
  initialCategories: SensitivityCategory[];
  onSave: (categories: SensitivityCategory[]) => void;
  onClose: () => void;
}

/** Modal "Aggiungi un avviso sul contenuto", stile Bluesky (CLAUDE.md #3):
 * usato sia per le immagini nel corpo del post (SensitiveImageNodeView) sia
 * per l'immagine di copertina (CoverImageUpload). */
export function ContentWarningModal({ initialCategories, onSave, onClose }: ContentWarningModalProps) {
  const [selected, setSelected] = useState<Set<SensitivityCategory>>(new Set(initialCategories));

  function toggle(category: SensitivityCategory) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(category)) next.delete(category);
      else next.add(category);
      return next;
    });
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-foreground/40 p-4"
      onClick={onClose}
      role="presentation"
    >
      <div
        className="w-full max-w-sm rounded-2xl border border-border bg-background p-5 shadow-lg"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
      >
        <h2 className="font-serif text-lg text-foreground">Aggiungi un avviso sul contenuto</h2>
        <p className="mt-1 text-sm text-muted">
          Aggiungi eventuali etichette di avviso applicabili ai contenuti che stai pubblicando.
        </p>
        {SENSITIVITY_CATEGORY_GROUPS.map((group) => (
          <fieldset key={group.heading} className="mt-4">
            <legend className="mb-2 text-sm font-medium text-foreground">{group.heading}</legend>
            <div className="space-y-1 rounded-lg border border-border">
              {group.categories.map((category) => (
                <label
                  key={category}
                  className="flex cursor-pointer items-center gap-2 border-b border-border px-3 py-2 text-sm text-foreground last:border-b-0"
                >
                  <input
                    type="checkbox"
                    checked={selected.has(category)}
                    onChange={() => toggle(category)}
                  />
                  {SENSITIVITY_CATEGORY_LABELS[category]}
                </label>
              ))}
            </div>
          </fieldset>
        ))}
        <div className="mt-5 flex justify-end">
          <Button
            type="button"
            onClick={() => {
              onSave(Array.from(selected));
              onClose();
            }}
          >
            Fatto
          </Button>
        </div>
      </div>
    </div>
  );
}
