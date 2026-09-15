"use client";

import { useState } from "react";
import { useTranslations } from "next-intl";

import { Button } from "@/components/ui/Button";
import { FieldGroup, Input, Label, TextArea } from "@/components/ui/Field";
import { Toggle } from "@/components/ui/Controls";
import {
  MAX_NOTE_AUTHOR_LENGTH,
  MAX_NOTE_DOI_LENGTH,
  MAX_NOTE_ISBN_LENGTH,
  MAX_NOTE_LENGTH,
  MAX_NOTE_PAGE_LENGTH,
  MAX_NOTE_TITLE_LENGTH,
  type StructuredNoteFields,
} from "@/lib/types";

export interface NoteModalValue extends StructuredNoteFields {
  content: string;
}

interface NoteModalProps {
  initial?: Partial<NoteModalValue>;
  onSave: (value: NoteModalValue) => void;
  onClose: () => void;
}

const EMPTY_STRUCTURED: StructuredNoteFields = { title: null, author: null, isbn: null, doi: null, page: null };

/** Modal "Nota", stesso stile visivo di `ContentWarningModal` (avviso sul
 * contenuto delle immagini) — overlay centrato, non un popover. Solo il
 * testo libero è obbligatorio; titolo e campi bibliografici sono tutti
 * facoltativi e compaiono insieme dietro il toggle. */
export function NoteModal({ initial, onSave, onClose }: NoteModalProps) {
  const t = useTranslations("RichTextEditor");
  const [content, setContent] = useState(initial?.content ?? "");
  const hasInitialDetails = Boolean(
    initial?.title || initial?.author || initial?.isbn || initial?.doi || initial?.page
  );
  const [detailsOn, setDetailsOn] = useState(hasInitialDetails);
  const [title, setTitle] = useState(initial?.title ?? "");
  const [author, setAuthor] = useState(initial?.author ?? "");
  const [isbn, setIsbn] = useState(initial?.isbn ?? "");
  const [doi, setDoi] = useState(initial?.doi ?? "");
  const [page, setPage] = useState(initial?.page ?? "");

  function handleSave() {
    const trimmed = content.trim();
    if (!trimmed) return;
    const structured: StructuredNoteFields = detailsOn
      ? {
          title: title.trim() || null,
          author: author.trim() || null,
          isbn: isbn.trim() || null,
          doi: doi.trim() || null,
          page: page.trim() || null,
        }
      : EMPTY_STRUCTURED;
    onSave({ content: trimmed, ...structured });
    onClose();
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
        <h2 className="font-serif text-lg text-foreground">{t("noteModalTitle")}</h2>
        <p className="mt-1 text-sm text-muted">{t("noteModalSubtitle")}</p>

        <FieldGroup className="mt-4 mb-0">
          <Label htmlFor="note-content">{t("noteContentLabel")}</Label>
          <TextArea
            id="note-content"
            autoFocus
            rows={3}
            maxLength={MAX_NOTE_LENGTH}
            value={content}
            onChange={(e) => setContent(e.target.value)}
            placeholder={t("notePlaceholder")}
          />
        </FieldGroup>

        <div className="mt-4 border-t border-border pt-4">
          <Toggle checked={detailsOn} onChange={setDetailsOn} label={t("noteDetailsToggle")} />
        </div>

        {detailsOn && (
          <div className="mt-3 flex flex-col gap-3">
            <FieldGroup className="mb-0">
              <Label htmlFor="note-title">{t("noteTitleLabel")}</Label>
              <Input
                id="note-title"
                maxLength={MAX_NOTE_TITLE_LENGTH}
                value={title}
                onChange={(e) => setTitle(e.target.value)}
              />
            </FieldGroup>
            <FieldGroup className="mb-0">
              <Label htmlFor="note-author">{t("noteAuthorLabel")}</Label>
              <Input
                id="note-author"
                maxLength={MAX_NOTE_AUTHOR_LENGTH}
                value={author}
                onChange={(e) => setAuthor(e.target.value)}
              />
            </FieldGroup>
            <div className="grid grid-cols-2 gap-3">
              <FieldGroup className="mb-0">
                <Label htmlFor="note-isbn">{t("noteIsbnLabel")}</Label>
                <Input
                  id="note-isbn"
                  maxLength={MAX_NOTE_ISBN_LENGTH}
                  value={isbn}
                  onChange={(e) => setIsbn(e.target.value)}
                />
              </FieldGroup>
              <FieldGroup className="mb-0">
                <Label htmlFor="note-page">{t("notePageLabel")}</Label>
                <Input
                  id="note-page"
                  maxLength={MAX_NOTE_PAGE_LENGTH}
                  value={page}
                  onChange={(e) => setPage(e.target.value)}
                />
              </FieldGroup>
            </div>
            <FieldGroup className="mb-0">
              <Label htmlFor="note-doi">{t("noteDoiLabel")}</Label>
              <Input
                id="note-doi"
                maxLength={MAX_NOTE_DOI_LENGTH}
                placeholder="10.xxxx/xxxxx"
                value={doi}
                onChange={(e) => setDoi(e.target.value)}
              />
            </FieldGroup>
          </div>
        )}

        <div className="mt-5 flex items-center gap-4">
          <Button type="button" disabled={!content.trim()} onClick={handleSave}>
            {t("done")}
          </Button>
          <button type="button" onClick={onClose} className="text-[13px] text-muted hover:text-foreground">
            {t("cancel")}
          </button>
        </div>
      </div>
    </div>
  );
}
