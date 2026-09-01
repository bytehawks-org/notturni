"use client";

import Link from "next/link";
import { useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { RichTextEditor } from "@/components/editor/RichTextEditor";
import type { PostNote, PostTranslationSummary } from "@/lib/types";

const languageNames = new Intl.DisplayNames(["it"], { type: "language" });

function localeName(locale: string): string {
  try {
    const name = languageNames.of(locale);
    return name ? name.charAt(0).toUpperCase() + name.slice(1) : locale;
  } catch {
    return locale;
  }
}

interface TranslationsBarProps {
  currentPostId: string;
  currentLocale: string;
  blogSlug: string;
  /** Solo le traduzioni pubblicamente visibili (l'endpoint le filtra così) —
   * per questo il post corrente va sempre mostrato a parte: se è una bozza
   * non compare in questa lista, nemmeno se stesso. */
  translations: PostTranslationSummary[];
  /** Lingue di fallback del profilo (dashboard/profilo): popolano il
   * selettore lingua qui sotto invece di dover scrivere la sigla a mano. */
  suggestedLocales: string[];
  authFetch: <T>(fn: (token: string) => Promise<T>) => Promise<T>;
  onAddTranslation: (payload: {
    slug: string;
    locale: string;
    title: string;
    content: string;
    notes: PostNote[];
  }) => Promise<void>;
}

/** Riga leggera, senza riquadro: le lingue già tradotte come pillole, più
 * un modulo che si apre per aggiungerne una nuova. Vedi fika.bar per il
 * riferimento stilistico. */
export function TranslationsBar({
  currentPostId,
  currentLocale,
  blogSlug,
  translations,
  suggestedLocales,
  authFetch,
  onAddTranslation,
}: TranslationsBarProps) {
  const otherTranslations = translations.filter((t) => t.id !== currentPostId);
  const alreadyUsed = new Set([currentLocale, ...otherTranslations.map((t) => t.locale)]);
  const availableSuggestions = suggestedLocales.filter((l) => !alreadyUsed.has(l));
  const [adding, setAdding] = useState(false);
  const [trLocale, setTrLocale] = useState("");
  const [trSlug, setTrSlug] = useState("");
  const [trTitle, setTrTitle] = useState("");
  const [trContent, setTrContent] = useState("");
  const [trNotes, setTrNotes] = useState<PostNote[]>([]);
  const [trError, setTrError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function handleAdd(event: FormEvent) {
    event.preventDefault();
    setTrError(null);
    setSubmitting(true);
    try {
      await onAddTranslation({
        slug: trSlug,
        locale: trLocale,
        title: trTitle,
        content: trContent,
        notes: trNotes,
      });
    } catch (err) {
      setTrError(err instanceof Error ? err.message : "Errore imprevisto.");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="mt-10 border-t border-border/60 pt-6">
      <div className="flex flex-wrap items-center gap-x-1.5 gap-y-2 text-sm">
        <span className="text-muted">Anche in</span>
        <span className="text-foreground underline underline-offset-4">{localeName(currentLocale)}</span>
        {otherTranslations.map((t) => (
          <span key={t.id} className="flex items-center gap-1.5">
            <span className="text-border">·</span>
            <Link
              href={`/dashboard/blogs/${blogSlug}/posts/${t.id}`}
              className="text-foreground/70 hover:text-primary"
            >
              {localeName(t.locale)} ✓
            </Link>
          </span>
        ))}
        {!adding && (
          <button
            type="button"
            onClick={() => setAdding(true)}
            className="ml-1 text-primary hover:underline"
          >
            + Aggiungi lingua
          </button>
        )}
      </div>

      {adding && (
        <form onSubmit={handleAdd} className="mt-6">
          <div className="mb-4 flex flex-wrap items-center gap-4">
            {availableSuggestions.length > 0 ? (
              <select
                required
                value={trLocale}
                onChange={(e) => setTrLocale(e.target.value)}
                className="rounded-md border border-border bg-background px-3 py-1.5 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-primary/40"
              >
                <option value="" disabled>
                  Lingua…
                </option>
                {availableSuggestions.map((code) => (
                  <option key={code} value={code}>
                    {localeName(code)}
                  </option>
                ))}
                <option value="__other__">Un&apos;altra lingua…</option>
              </select>
            ) : null}
            {(availableSuggestions.length === 0 || trLocale === "__other__") && (
              <input
                required
                maxLength={2}
                minLength={2}
                placeholder="Lingua (es. en)"
                value={trLocale === "__other__" ? "" : trLocale}
                onChange={(e) => setTrLocale(e.target.value.toLowerCase())}
                className="w-32 border-0 border-b border-border bg-transparent py-1 text-sm text-foreground placeholder:text-muted focus:border-primary focus:outline-none"
              />
            )}
            <input
              required
              placeholder="Slug"
              value={trSlug}
              onChange={(e) => setTrSlug(e.target.value)}
              className="w-48 border-0 border-b border-border bg-transparent py-1 text-sm text-foreground placeholder:text-muted focus:border-primary focus:outline-none"
            />
          </div>
          <input
            required
            placeholder="Titolo"
            value={trTitle}
            onChange={(e) => setTrTitle(e.target.value)}
            className="mb-4 w-full border-0 bg-transparent font-serif text-2xl font-semibold text-foreground placeholder:text-muted focus:outline-none"
          />
          <RichTextEditor
            value={trContent}
            onChange={setTrContent}
            blogSlug={blogSlug}
            authFetch={authFetch}
            notes={trNotes}
            onNotesChange={setTrNotes}
          />
          {trError && (
            <div className="mt-4">
              <Alert kind="error">{trError}</Alert>
            </div>
          )}
          <div className="mt-4 flex gap-3">
            <Button type="submit" disabled={submitting}>
              {submitting ? "Creazione…" : "Crea traduzione"}
            </Button>
            <Button type="button" variant="ghost" onClick={() => setAdding(false)}>
              Annulla
            </Button>
          </div>
        </form>
      )}
    </div>
  );
}
