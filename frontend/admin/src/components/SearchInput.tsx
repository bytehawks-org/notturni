"use client";

import { useEffect, useState } from "react";

import { Input } from "@/components/ui/Field";

const DEBOUNCE_MS = 300;

/** Campo di ricerca condiviso dalle sezioni admin (Pagine/Utenti/Blog):
 * notifica onChange con un debounce, per non rifare una richiesta ad ogni
 * carattere digitato. */
export function SearchInput({
  value,
  onChange,
  placeholder,
}: {
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
}) {
  const [draft, setDraft] = useState(value);

  useEffect(() => {
    const timeout = window.setTimeout(() => onChange(draft), DEBOUNCE_MS);
    return () => window.clearTimeout(timeout);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [draft]);

  return (
    <Input
      type="search"
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      placeholder={placeholder}
      className="max-w-xs"
      aria-label={placeholder}
    />
  );
}
