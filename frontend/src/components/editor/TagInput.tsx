"use client";

import { useState, type KeyboardEvent } from "react";

import { MAX_TAGS_PER_POST } from "@/lib/types";

interface TagInputProps {
  value: string[];
  onChange: (tags: string[]) => void;
}

/** Campo dedicato per i tag (max 5) — si sommano agli eventuali #hashtag
 * scritti nel testo del post, il tetto è sul totale (vedi backend). */
export function TagInput({ value, onChange }: TagInputProps) {
  const [draft, setDraft] = useState("");

  function addTag() {
    const normalized = draft.trim().toLowerCase().replace(/^#/, "");
    if (!normalized) return;
    if (value.includes(normalized)) {
      setDraft("");
      return;
    }
    onChange([...value, normalized]);
    setDraft("");
  }

  function handleKeyDown(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      addTag();
    } else if (e.key === "Backspace" && draft === "" && value.length > 0) {
      onChange(value.slice(0, -1));
    }
  }

  const atLimit = value.length >= MAX_TAGS_PER_POST;

  return (
    <div className="flex flex-wrap items-center gap-2">
      {value.map((tag) => (
        <span
          key={tag}
          className="inline-flex items-center gap-1 rounded-full bg-primary/10 px-3 py-1 text-sm text-primary"
        >
          #{tag}
          <button
            type="button"
            onClick={() => onChange(value.filter((t) => t !== tag))}
            aria-label={`Rimuovi tag ${tag}`}
            className="text-primary/60 hover:text-primary"
          >
            ×
          </button>
        </span>
      ))}
      {!atLimit && (
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={handleKeyDown}
          onBlur={addTag}
          placeholder={value.length === 0 ? "Aggiungi un tag…" : ""}
          className="min-w-24 flex-1 border-0 bg-transparent py-1 text-sm text-foreground placeholder:text-muted focus:outline-none"
        />
      )}
      {atLimit && <span className="text-xs text-muted">massimo {MAX_TAGS_PER_POST}</span>}
    </div>
  );
}
