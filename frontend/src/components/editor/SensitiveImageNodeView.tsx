"use client";

import { NodeViewWrapper, ReactNodeViewRenderer, type NodeViewProps } from "@tiptap/react";
import { useState } from "react";

import {
  buildSensitiveTitle,
  isFlaggedSensitive,
  parseSensitivityCategories,
  type SensitivityCategory,
} from "@/lib/content-media";

import { ContentWarningModal } from "./ContentWarningModal";
import { CloseIcon, PencilIcon, ShieldIcon } from "./icons";

/** Immagine incorporata nel corpo del post, stile Bluesky (CLAUDE.md #2/#3):
 * pillola "+ ALT" in alto a sinistra per il testo alternativo, "X" per
 * rimuoverla, e un pulsante in sovraimpressione in basso che apre il modal
 * di avviso sui contenuti. Se segnalata sensibile (automoderazione o scelta
 * manuale), resta sfocata finché non ci si clicca sopra — stessa idea del
 * trucco CSS usato sulla pagina pubblica (frontend/src/lib/markdown.ts), qui
 * con lo stato di React perché siamo già dentro l'app, non in HTML statico. */
function ImageNodeView({ node, updateAttributes, deleteNode, selected }: NodeViewProps) {
  const src = node.attrs.src as string;
  const alt = (node.attrs.alt as string | null) ?? "";
  const title = node.attrs.title as string | null;

  const categories = parseSensitivityCategories(title);
  const sensitive = isFlaggedSensitive(title);
  const [revealed, setRevealed] = useState(false);
  const [editingAlt, setEditingAlt] = useState(false);
  const [altDraft, setAltDraft] = useState(alt);
  const [showWarningModal, setShowWarningModal] = useState(false);

  const blurred = sensitive && !revealed;

  function saveAlt() {
    updateAttributes({ alt: altDraft.trim() || null });
    setEditingAlt(false);
  }

  function saveCategories(next: SensitivityCategory[]) {
    updateAttributes({ title: next.length > 0 ? buildSensitiveTitle(next) : null });
    setRevealed(false);
  }

  return (
    <NodeViewWrapper
      as="span"
      className={`relative my-2 inline-block max-w-full align-top ${selected ? "outline outline-2 outline-primary" : ""}`}
    >
      {/* eslint-disable-next-line @next/next/no-img-element -- URL storage esterno, non ottimizzabile da next/image senza configurare i domini */}
      <img
        src={src}
        alt={alt}
        onClick={() => blurred && setRevealed(true)}
        className={`block max-h-[480px] max-w-full rounded-lg ${blurred ? "cursor-pointer blur-2xl" : ""}`}
      />

      {blurred && (
        <div className="absolute inset-0 flex items-center justify-center rounded-lg bg-foreground/10">
          <span className="rounded-full bg-foreground/70 px-3 py-1 text-sm text-background">
            Contenuto sensibile — clicca per vedere
          </span>
        </div>
      )}

      <button
        type="button"
        onClick={() => {
          setAltDraft(alt);
          setEditingAlt(true);
        }}
        className="absolute left-2 top-2 flex items-center gap-1 rounded-full bg-foreground/70 px-2.5 py-1 text-xs font-medium text-background"
      >
        {alt ? <PencilIcon /> : null}
        {alt ? "ALT" : "+ ALT"}
      </button>

      <button
        type="button"
        onClick={() => deleteNode()}
        title="Rimuovi immagine"
        className="absolute right-2 top-2 flex h-7 w-7 items-center justify-center rounded-full bg-foreground/70 text-background"
      >
        <CloseIcon />
      </button>

      <button
        type="button"
        onClick={() => setShowWarningModal(true)}
        className="absolute bottom-2 left-1/2 flex -translate-x-1/2 items-center gap-1.5 whitespace-nowrap rounded-full bg-foreground/70 px-3 py-1 text-xs font-medium text-background"
      >
        <ShieldIcon />
        {sensitive ? "Avviso sul contenuto" : "Aggiungi un avviso"}
      </button>

      {editingAlt && (
        <div
          onClick={(e) => e.stopPropagation()}
          className="absolute inset-x-2 bottom-2 rounded-lg border border-border bg-background p-3 shadow-lg"
        >
          <label className="mb-1 block text-xs font-medium text-muted">Testo alternativo</label>
          <textarea
            autoFocus
            rows={2}
            value={altDraft}
            onChange={(e) => setAltDraft(e.target.value)}
            className="w-full rounded-md border border-border bg-background p-2 text-sm text-foreground"
          />
          <div className="mt-2 flex justify-end gap-3 text-xs">
            <button type="button" onClick={() => setEditingAlt(false)} className="text-muted">
              Annulla
            </button>
            <button type="button" onClick={saveAlt} className="font-medium text-primary">
              Salva
            </button>
          </div>
        </div>
      )}

      {showWarningModal && (
        <ContentWarningModal
          initialCategories={categories}
          onSave={saveCategories}
          onClose={() => setShowWarningModal(false)}
        />
      )}
    </NodeViewWrapper>
  );
}

export function sensitiveImageNodeView() {
  return ReactNodeViewRenderer(ImageNodeView);
}
