"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";

import { Button } from "@/components/ui/Button";

export interface FragmentSelection {
  text: string;
  words: number;
  /** Quota del testo del post selezionata (0..1); oltre MAX_FRAGMENT_RATIO non si salva. */
  ratio: number;
  top: number;
  left: number;
}

export interface FragmentMenuProps {
  sel: FragmentSelection;
  /** Selezione che tocca frammenti già salvati → l'azione principale è "Rimuovi". */
  saved: boolean;
  signedIn: boolean;
  busy: boolean;
  onSave: (isPublic: boolean) => void;
  onRemove: () => void;
  onCopyLink: () => void;
  onQuote: () => void;
  onClose: () => void;
}

/**
 * Mockup 1a/1b: menu flottante ancorato sopra la selezione (desktop) e
 * bottom-sheet (mobile). Il chiamante (FragmentReader) possiede la logica
 * di selezione/evidenziazione — vedi lib/highlight-fragments.ts.
 */
export function FragmentMenu({ sel, saved, signedIn, busy, onSave, onRemove, onCopyLink, onQuote, onClose }: FragmentMenuProps) {
  const t = useTranslations("Fragment");
  const tooLong = sel.ratio > 0.15;
  const meta = t("meta", { words: sel.words, ratio: Math.round(sel.ratio * 100) });
  const primary = !signedIn ? (
    <Link href="/login" className="fragment-menu-link">
      {t("signInToSave")}
    </Link>
  ) : saved ? (
    <button type="button" className="fragment-menu-button fragment-menu-button--remove" onClick={onRemove} disabled={busy}>
      {busy ? t("removing") : t("remove")}
    </button>
  ) : (
    <>
      <button type="button" className="fragment-menu-button" onClick={() => onSave(false)} disabled={busy || tooLong}>
        {busy ? t("saving") : t("save")}
      </button>
      <button type="button" className="fragment-menu-button" onClick={() => onSave(true)} disabled={busy || tooLong}>
        {t("savePublic")}
      </button>
    </>
  );

  return (
    <>
      <div
        className="fragment-menu hidden md:flex"
        style={{ left: sel.left, top: sel.top }}
        role="toolbar"
        onMouseDown={(e) => e.preventDefault()}
      >
        <div className="fragment-menu-actions">
          {primary}
          <button type="button" className="fragment-menu-button text-muted!" onClick={onCopyLink}>
            {t("copyLink")}
          </button>
          <button type="button" className="fragment-menu-button text-muted!" onClick={onQuote}>
            {t("quote")}
          </button>
          <span className="fragment-menu-hint border-l border-border pl-3">{tooLong ? t("tooLong") : meta}</span>
        </div>
      </div>
      <div
        className="fixed inset-x-0 bottom-0 z-50 rounded-t-[22px] border-t border-border bg-surface px-5 pb-7 pt-2.5 shadow-soft md:hidden"
        role="dialog"
        onMouseDown={(e) => e.preventDefault()}
      >
        <span className="mx-auto mb-3.5 block h-1 w-9 rounded-full bg-border" />
        <div className="flex items-baseline justify-between">
          <span className="font-serif text-[17px]">{t("selectedText")}</span>
          <span className="text-xs text-muted">{tooLong ? t("tooLong") : meta}</span>
        </div>
        <p className="my-2 mb-3.5 line-clamp-2 border-l-[3px] border-[var(--fragment-highlight-border)] pl-2.5 text-sm leading-relaxed text-muted">
          “{sel.text}”
        </p>
        <div className="grid grid-cols-2 gap-2.5">
          {!signedIn ? (
            <Link href="/login" className="contents">
              <Button size="lg">{t("signInToSave")}</Button>
            </Link>
          ) : saved ? (
            <Button variant="danger" size="lg" onClick={onRemove} disabled={busy}>
              {t("removeShort")}
            </Button>
          ) : (
            <Button size="lg" disabled={busy || tooLong} onClick={() => onSave(false)}>
              {t("save")}
            </Button>
          )}
          <Button variant="secondary" size="lg" onClick={onCopyLink}>
            {t("copyLink")}
          </Button>
        </div>
        <div className="mt-3.5 flex justify-center gap-6 text-[13px] text-muted">
          {signedIn && !saved && (
            <button type="button" onClick={() => onSave(true)} disabled={busy || tooLong}>
              {t("savePublic")}
            </button>
          )}
          <button type="button" onClick={onQuote}>
            {t("quote")}
          </button>
          <button type="button" onClick={onClose}>
            {t("close")}
          </button>
        </div>
      </div>
    </>
  );
}
