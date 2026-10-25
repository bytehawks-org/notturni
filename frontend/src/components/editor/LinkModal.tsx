"use client";

import { useEffect, useRef, useState } from "react";
import { useTranslations } from "next-intl";

import { Button } from "@/components/ui/Button";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { api } from "@/lib/api";
import { SITE_URL } from "@/lib/site";

export interface LinkModalValue {
  url: string;
  /** Testo mostrato al posto dell'URL; assente → si mostra l'URL. */
  text: string;
}

interface LinkModalProps {
  initial: LinkModalValue;
  /** `true` se si sta modificando un link esistente: compare «Rimuovi link». */
  editing: boolean;
  onSave: (value: LinkModalValue) => void;
  onRemove: () => void;
  onClose: () => void;
}

interface Hit {
  kind: "post" | "blog" | "user";
  key: string;
  title: string;
  meta: string;
  url: string;
}

/** URL utilizzabile come `href`: http(s), mailto, ancore e percorsi del sito restano com'è; un indirizzo senza
 * schema (`example.com/x`) diventa https. Schemi pericolosi (`javascript:`, `data:`…) non passano mai. */
export function normalizeLinkUrl(raw: string): string | null {
  const value = raw.trim();
  if (!value) return null;
  if (/^(https?:\/\/|mailto:)/i.test(value) || value.startsWith("/") || value.startsWith("#")) return value;
  if (/^[a-z][a-z0-9+.-]*:(?!\d)/i.test(value)) return null; // altri schemi (javascript:, data:…): no; `host:8080` è un indirizzo
  return `https://${value}`;
}

const MIN_QUERY = 2;
const DEBOUNCE_MS = 250;

/** Modal «Link», stesso stile di `NoteModal`/`ContentWarningModal` (overlay centrato). Oltre all'URL ha il testo
 * da mostrare (se vuoto si mostra l'URL) e una ricerca nella piattaforma — post, spazi, utenti — per collegare
 * un contenuto esistente senza incollarne l'indirizzo. I link ai contenuti sono assoluti (`SITE_URL`), così
 * funzionano anche nelle email, negli export e nei repository sincronizzati. */
export function LinkModal({ initial, editing, onSave, onRemove, onClose }: LinkModalProps) {
  const t = useTranslations("RichTextEditor");
  const [url, setUrl] = useState(initial.url);
  const [text, setText] = useState(initial.text);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<Hit[] | null>(null);
  const [searching, setSearching] = useState(false);
  const latest = useRef(0);

  useEffect(() => {
    const q = query.trim().replace(/^@/, "");
    if (q.length < MIN_QUERY) return;
    const ticket = ++latest.current;
    const timer = setTimeout(() => {
      setSearching(true);
      Promise.allSettled([api.search.posts(q), api.search.blogs(q), api.search.users(q)])
        .then(([posts, blogs, users]) => {
          if (ticket !== latest.current) return; // risposta di una ricerca superata
          const out: Hit[] = [];
          if (posts.status === "fulfilled")
            for (const p of posts.value)
              out.push({ kind: "post", key: `p${p.id}`, title: p.title, meta: p.blog_slug, url: `${SITE_URL}${p.permalink}` });
          if (blogs.status === "fulfilled")
            for (const b of blogs.value)
              out.push({ kind: "blog", key: `b${b.slug}`, title: b.title, meta: b.slug, url: `${SITE_URL}/${b.slug}` });
          if (users.status === "fulfilled")
            for (const u of users.value)
              out.push({
                kind: "user",
                key: `u${u.username}`,
                title: u.display_name || u.username,
                meta: `@${u.username}`,
                url: `${SITE_URL}/u/${u.username}`,
              });
          setHits(out);
        })
        .finally(() => ticket === latest.current && setSearching(false));
    }, DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [query]);

  const normalized = normalizeLinkUrl(url);
  const invalid = url.trim() !== "" && normalized === null;
  const visibleHits = query.trim().replace(/^@/, "").length >= MIN_QUERY ? hits : null;

  function pick(hit: Hit) {
    setUrl(hit.url);
    if (!text.trim()) setText(hit.title);
    setQuery("");
    setHits(null);
  }

  function handleSave() {
    if (!normalized) return;
    onSave({ url: normalized, text: text.trim() });
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
        onKeyDown={(e) => e.key === "Escape" && onClose()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="link-modal-title"
      >
        <h2 id="link-modal-title" className="font-serif text-lg text-foreground">
          {t("linkModalTitle")}
        </h2>
        <p className="mt-1 text-sm text-muted">{t("linkModalSubtitle")}</p>

        <FieldGroup className="mt-4 mb-0">
          <Label htmlFor="link-url">{t("linkUrlLabel")}</Label>
          <Input
            id="link-url"
            autoFocus
            inputMode="url"
            placeholder="https://…"
            maxLength={2048}
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && (e.preventDefault(), handleSave())}
            aria-invalid={invalid}
          />
          {invalid && <p className="mt-1 text-[12px] text-danger">{t("linkUrlInvalid")}</p>}
        </FieldGroup>

        <FieldGroup className="mt-3 mb-0">
          <Label htmlFor="link-text" hint={t("linkTextHint")}>
            {t("linkTextLabel")}
          </Label>
          <Input
            id="link-text"
            maxLength={300}
            placeholder={normalized ?? t("linkTextPlaceholder")}
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && (e.preventDefault(), handleSave())}
          />
        </FieldGroup>

        <div className="mt-4 border-t border-border pt-4">
          <FieldGroup className="mb-0">
            <Label htmlFor="link-search">{t("linkSearchLabel")}</Label>
            <Input
              id="link-search"
              type="search"
              autoComplete="off"
              placeholder={t("linkSearchPlaceholder")}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </FieldGroup>
          {searching && <p className="mt-2 text-[12px] text-muted">{t("linkSearching")}</p>}
          {visibleHits && !searching && visibleHits.length === 0 && (
            <p className="mt-2 text-[12px] text-muted">{t("linkNoResults")}</p>
          )}
          {visibleHits && visibleHits.length > 0 && (
            <ul className="mt-2 flex max-h-48 flex-col overflow-y-auto rounded-lg border border-border" role="listbox">
              {visibleHits.map((hit) => (
                <li key={hit.key} role="option" aria-selected={url === hit.url}>
                  <button
                    type="button"
                    onClick={() => pick(hit)}
                    className="flex w-full items-baseline gap-2 px-3 py-2 text-left text-sm hover:bg-surface"
                  >
                    <span className="shrink-0 rounded-full border border-border px-1.5 text-[10px] uppercase tracking-wide text-muted">
                      {t(`linkKind.${hit.kind}`)}
                    </span>
                    <span className="min-w-0 flex-1 truncate text-foreground">{hit.title}</span>
                    <span className="shrink-0 text-[12px] text-muted">{hit.meta}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className="mt-5 flex items-center gap-4">
          <Button type="button" disabled={!normalized} onClick={handleSave}>
            {t("done")}
          </Button>
          <button type="button" onClick={onClose} className="text-[13px] text-muted hover:text-foreground">
            {t("cancel")}
          </button>
          {editing && (
            <button type="button" onClick={onRemove} className="ml-auto text-[13px] text-danger hover:underline">
              {t("linkRemove")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
