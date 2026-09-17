"use client";

import { useState } from "react";
import { useTranslations } from "next-intl";

import { Button } from "@/components/ui/Button";
import { FieldGroup, Input, Label, TextArea } from "@/components/ui/Field";
import { SegmentedControl } from "@/components/ui/Controls";
import { Toggle } from "@/components/ui/Controls";
import {
  MAX_NOTE_AUTHOR_LENGTH,
  MAX_NOTE_DOI_LENGTH,
  MAX_NOTE_ISBN_LENGTH,
  MAX_NOTE_ISSUED_LENGTH,
  MAX_NOTE_LENGTH,
  MAX_NOTE_PAGE_LENGTH,
  MAX_NOTE_SOURCE_LENGTH,
  MAX_NOTE_TITLE_LENGTH,
  MAX_NOTE_URL_LENGTH,
  type NoteKind,
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

const NOTE_KINDS: NoteKind[] = ["note", "book", "article", "web"];

const EMPTY_STRUCTURED: StructuredNoteFields = {
  title: null,
  author: null,
  kind: null,
  source: null,
  issued: null,
  isbn: null,
  doi: null,
  url: null,
  page: null,
};

/** Modal "Nota", stesso stile visivo di `ContentWarningModal` (avviso sul
 * contenuto delle immagini) — overlay centrato, non un popover. Solo il
 * testo libero è obbligatorio; titolo e campi bibliografici sono tutti
 * facoltativi e compaiono insieme dietro il toggle. */
export function NoteModal({ initial, onSave, onClose }: NoteModalProps) {
  const t = useTranslations("RichTextEditor");
  const tb = useTranslations("Bibliography");
  const [content, setContent] = useState(initial?.content ?? "");
  const hasInitialDetails = Boolean(
    initial?.title ||
      initial?.author ||
      initial?.kind ||
      initial?.source ||
      initial?.issued ||
      initial?.isbn ||
      initial?.doi ||
      initial?.url ||
      initial?.page
  );
  const [detailsOn, setDetailsOn] = useState(hasInitialDetails);
  const [title, setTitle] = useState(initial?.title ?? "");
  const [author, setAuthor] = useState(initial?.author ?? "");
  const [kind, setKind] = useState<NoteKind | "">(initial?.kind ?? "");
  const [source, setSource] = useState(initial?.source ?? "");
  const [issued, setIssued] = useState(initial?.issued ?? "");
  const [isbn, setIsbn] = useState(initial?.isbn ?? "");
  const [doi, setDoi] = useState(initial?.doi ?? "");
  const [url, setUrl] = useState(initial?.url ?? "");
  const [page, setPage] = useState(initial?.page ?? "");

  function handleSave() {
    const trimmed = content.trim();
    if (!trimmed) return;
    const structured: StructuredNoteFields = detailsOn
      ? {
          title: title.trim() || null,
          author: author.trim() || null,
          kind: kind || null,
          source: source.trim() || null,
          issued: issued.trim() || null,
          isbn: isbn.trim() || null,
          doi: doi.trim() || null,
          url: url.trim() || null,
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
            <div className="flex flex-col gap-1.5">
              <Label htmlFor="note-kind">{t("noteKindLabel")}</Label>
              <SegmentedControl<NoteKind | "">
                value={kind}
                options={[
                  { value: "", label: tb("kind.note") },
                  ...NOTE_KINDS.filter((k) => k !== "note").map((k) => ({ value: k, label: tb(`kind.${k}`) })),
                ]}
                onChange={setKind}
              />
            </div>
            <FieldGroup className="mb-0">
              <Label htmlFor="note-source">{t("noteSourceLabel")}</Label>
              <Input
                id="note-source"
                maxLength={MAX_NOTE_SOURCE_LENGTH}
                value={source}
                onChange={(e) => setSource(e.target.value)}
              />
            </FieldGroup>
            <div className="grid grid-cols-2 gap-3">
              <FieldGroup className="mb-0">
                <Label htmlFor="note-issued">{t("noteIssuedLabel")}</Label>
                <Input
                  id="note-issued"
                  maxLength={MAX_NOTE_ISSUED_LENGTH}
                  value={issued}
                  onChange={(e) => setIssued(e.target.value)}
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
            <FieldGroup className="mb-0">
              <Label htmlFor="note-url">{t("noteUrlLabel")}</Label>
              <Input
                id="note-url"
                maxLength={MAX_NOTE_URL_LENGTH}
                type="url"
                placeholder="https://…"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
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
