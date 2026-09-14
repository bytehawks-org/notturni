"use client";
import { useTranslations } from "next-intl";
import { Button } from "../ui/Button";

export interface FragmentSelection { text: string; words: number; ratio: number; rect: DOMRect }

/**
 * Desktop: floating menu anchored above the selection (position: fixed, from `rect`).
 * Mobile (<md): bottom sheet. Caller (FragmentReader) owns selection logic — see lib/highlight-fragments.ts.
 */
export function FragmentMenu({ sel, saved, onSave, onRemove, onCopyLink, onQuote, onClose }: {
  sel: FragmentSelection; saved: boolean; onSave: () => void; onRemove: () => void; onCopyLink: () => void; onQuote: () => void; onClose: () => void;
}) {
  const t = useTranslations("Fragment");
  const tooLong = sel.ratio > 0.15;
  const meta = t("meta", { words: sel.words, ratio: Math.round(sel.ratio * 100) });
  return (
    <>
      <div className="fragment-menu hidden md:flex" style={{ left: sel.rect.left + sel.rect.width / 2, top: sel.rect.top }} role="toolbar">
        <div className="fragment-menu-actions">
          {saved ? <button className="fragment-menu-button fragment-menu-button--remove" onClick={onRemove}>{t("remove")}</button>
                 : <button className="fragment-menu-button" onClick={onSave} disabled={tooLong}>{t("save")}</button>}
          <button className="fragment-menu-button text-muted!" onClick={onCopyLink}>{t("copyLink")}</button>
          <button className="fragment-menu-button text-muted!" onClick={onQuote}>{t("quote")}</button>
          <span className="fragment-menu-hint border-l border-border pl-3">{tooLong ? t("tooLong") : meta}</span>
        </div>
      </div>
      <div className="fixed inset-x-0 bottom-0 z-50 rounded-t-[22px] border-t border-border bg-surface px-5 pb-7 pt-2.5 shadow-soft md:hidden" role="dialog">
        <span className="mx-auto mb-3.5 block h-1 w-9 rounded-full bg-border" />
        <div className="flex items-baseline justify-between"><span className="font-serif text-[17px]">{t("selectedText")}</span><span className="text-xs text-muted">{meta}</span></div>
        <p className="my-2 mb-3.5 border-l-[3px] border-[var(--fragment-highlight-border)] pl-2.5 text-sm leading-relaxed text-muted line-clamp-2">“{sel.text}”</p>
        <div className="grid grid-cols-2 gap-2.5">
          {saved ? <Button variant="danger" size="lg" onClick={onRemove}>{t("removeShort")}</Button> : <Button size="lg" disabled={tooLong} onClick={onSave}>{t("save")}</Button>}
          <Button variant="secondary" size="lg" onClick={onCopyLink}>{t("copyLink")}</Button>
        </div>
        <div className="mt-3.5 flex justify-center gap-6 text-[13px] text-muted"><button onClick={onQuote}>{t("quote")}</button><button onClick={onClose}>{t("close")}</button></div>
      </div>
    </>
  );
}
