"use client";

import { useTranslations } from "next-intl";
import ImageExtension from "@tiptap/extension-image";
import LinkExtension from "@tiptap/extension-link";
import Placeholder from "@tiptap/extension-placeholder";
import { TextSelection } from "@tiptap/pm/state";
import { Table } from "@tiptap/extension-table";
import TableCell from "@tiptap/extension-table-cell";
import TableHeader from "@tiptap/extension-table-header";
import TableRow from "@tiptap/extension-table-row";
import type { Editor } from "@tiptap/react";
import { EditorContent, useEditor, useEditorState } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Image from "next/image";
import { useCallback, useEffect, useRef, useState } from "react";
import { Markdown, type MarkdownStorage } from "tiptap-markdown";

import { ApiClientError, api } from "@/lib/api";
import { MAX_NOTE_LENGTH, type PostNote } from "@/lib/types";

import {
  BulletListIcon,
  ImageIcon,
  LinkIcon,
  NoteIcon,
  OrderedListIcon,
  QuoteIcon,
  RedoIcon,
  TableIcon,
  UndoIcon,
} from "./icons";
import { LinkPreviewCard } from "./LinkPreviewCard";
import { NoteModal, type NoteModalValue } from "./NoteModal";
import { sensitiveImageNodeView } from "./SensitiveImageNodeView";

export interface RichTextEditorProps {
  /** Contenuto iniziale in Markdown (il backend salva/legge solo Markdown). */
  value: string;
  onChange: (markdown: string) => void;
  /** Slug del blog: serve per caricare le immagini incorporate sul backend di storage e
   * per l'autocomplete delle @menzioni. Assente per contenuti non legati a un
   * blog (es. pagine statiche di piattaforma): in quel caso il pulsante
   * "Immagine" e le @menzioni sono disattivati. */
  blogSlug?: string;
  authFetch: <T>(fn: (token: string) => Promise<T>) => Promise<T>;
  placeholder?: string;
  /** Note a piè di pagina del post (todo/EDITOR.md). Assenti per contenuti
   * che non le prevedono (es. pagine statiche, CLAUDE.md #1): in quel caso
   * il pulsante "Nota" e l'elenco delle note non vengono mostrati. */
  notes?: PostNote[];
  onNotesChange?: (notes: PostNote[]) => void;
  /** Controlli aggiuntivi (es. categoria, stato di pubblicazione) mostrati
   * sopra i pulsanti di formattazione, dentro la stessa toolbar — così
   * restano visibili insieme ad essa. RichTextEditor resta agnostico sul
   * loro contenuto: chi lo usa compone i propri controlli (es. `CategorySelect`). */
  toolbarEnd?: React.ReactNode;
  /** Se true (default), la toolbar resta visibile in cima allo schermo
   * durante lo scroll di un contenuto lungo. Da disattivare quando l'editor
   * è annidato in un form breve dentro una pagina più lunga (es. il modulo
   * "Aggiungi traduzione"), dove una seconda barra fissa sarebbe fuori
   * contesto. */
  stickyToolbar?: boolean;
}

// Pulsanti compatti per stare tutti su un'unica riga: la riga scorre in
// orizzontale (overflow-x-auto sul contenitore) solo se lo spazio non basta,
// invece di andare a capo su più righe come nella versione precedente.
function ToolbarButton({
  active,
  disabled,
  onClick,
  title,
  children,
}: {
  active?: boolean;
  disabled?: boolean;
  onClick: () => void;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      disabled={disabled}
      onMouseDown={(e) => e.preventDefault()} // non rubare il focus all'editor
      onClick={onClick}
      className={`inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-sm transition disabled:opacity-30 disabled:cursor-not-allowed [&>svg]:h-4 [&>svg]:w-4 ${
        active ? "bg-primary/10 text-primary" : "text-foreground/70 hover:bg-foreground/5 hover:text-foreground"
      }`}
    >
      {children}
    </button>
  );
}

function ToolbarDivider() {
  return <span className="mx-1 h-6 w-px shrink-0 bg-border" />;
}

interface MentionCandidate {
  username: string;
  display_name: string | null;
  avatar_url: string | null;
}

// La `@` deve essere a inizio riga o preceduta da uno spazio; poi 0..32
// caratteri del formato username (vedi app/domain/usernames.py).
const MENTION_TRIGGER_RE = /(?:^|\s)@([a-z0-9_-]{0,32})$/;

/** Autocomplete delle @menzioni nell'editor (todo/EDITOR.md): rileva la
 * digitazione di `@parola`, propone gli utenti del blog e, alla selezione,
 * inserisce `@username ` come testo semplice (il Markdown resta pulito, il
 * link si forma al rendering — vedi src/lib/markdown.ts). Se il blog ha le
 * menzioni disattivate l'endpoint non restituisce nulla e il menu non appare.
 * Senza `blogSlug` (contenuti non legati a un blog, es. pagine di
 * piattaforma) l'autocomplete resta sempre disattivato. */
function useMentionAutocomplete(
  editor: Editor | null,
  blogSlug: string | undefined,
  authFetch: <T>(fn: (token: string) => Promise<T>) => Promise<T>
) {
  const [anchor, setAnchor] = useState<{
    from: number;
    to: number;
    query: string;
    left: number;
    top: number;
  } | null>(null);
  const [items, setItems] = useState<MentionCandidate[]>([]);
  const [index, setIndex] = useState(0);

  const anchorRef = useRef(anchor);
  const itemsRef = useRef(items);
  const indexRef = useRef(index);
  useEffect(() => {
    anchorRef.current = anchor;
    itemsRef.current = items;
    indexRef.current = index;
  });

  const close = useCallback(() => {
    setAnchor(null);
    setItems([]);
    setIndex(0);
  }, []);

  const applyMention = useCallback(
    (candidate: MentionCandidate) => {
      const current = anchorRef.current;
      if (!editor || !current) return;
      editor
        .chain()
        .focus()
        .insertContentAt({ from: current.from, to: current.to }, `@${candidate.username} `)
        .run();
      close();
    },
    [editor, close]
  );

  useEffect(() => {
    if (!editor || !blogSlug) return;
    const detect = () => {
      const { selection } = editor.state;
      if (!selection.empty) return close();
      const { $from } = selection;
      const textBefore = $from.parent.textBetween(
        Math.max(0, $from.parentOffset - 60),
        $from.parentOffset,
        undefined,
        "￼"
      );
      const match = MENTION_TRIGGER_RE.exec(textBefore);
      if (!match) return close();
      const from = selection.from - match[1].length - 1;
      const coords = editor.view.coordsAtPos(from);
      setAnchor({ from, to: selection.from, query: match[1], left: coords.left, top: coords.bottom + 4 });
    };
    editor.on("selectionUpdate", detect);
    editor.on("update", detect);
    return () => {
      editor.off("selectionUpdate", detect);
      editor.off("update", detect);
    };
  }, [editor, blogSlug, close]);

  const query = anchor?.query;
  const open = anchor !== null;
  useEffect(() => {
    if (!open || !blogSlug) return;
    const handle = setTimeout(() => {
      authFetch((token) => api.blogs.mentionableUsers(token, blogSlug, query ?? ""))
        .then((list) => {
          setItems(list);
          setIndex(0);
        })
        .catch(() => setItems([]));
    }, 120);
    return () => clearTimeout(handle);
  }, [open, query, blogSlug, authFetch]);

  useEffect(() => {
    if (!editor) return;
    const dom = editor.view.dom;
    const onKeyDown = (event: KeyboardEvent) => {
      if (!anchorRef.current || itemsRef.current.length === 0) return;
      const count = itemsRef.current.length;
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setIndex((i) => (i + 1) % count);
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setIndex((i) => (i - 1 + count) % count);
      } else if (event.key === "Enter" || event.key === "Tab") {
        event.preventDefault();
        applyMention(itemsRef.current[indexRef.current]);
      } else if (event.key === "Escape") {
        event.preventDefault();
        close();
      }
    };
    dom.addEventListener("keydown", onKeyDown, true);
    return () => dom.removeEventListener("keydown", onKeyDown, true);
  }, [editor, applyMention, close]);

  if (!anchor || items.length === 0) return null;
  return (
    <ul
      style={{ position: "fixed", left: anchor.left, top: anchor.top, zIndex: 50 }}
      className="max-h-56 w-64 overflow-auto rounded-lg border border-border bg-surface py-1 text-sm shadow-soft"
    >
      {items.map((item, i) => (
        <li key={item.username}>
          <button
            type="button"
            onMouseDown={(e) => {
              e.preventDefault();
              applyMention(item);
            }}
            className={`flex w-full items-center gap-2 px-3 py-1.5 text-left ${
              i === index ? "bg-primary/10 text-foreground" : "text-foreground/80 hover:bg-foreground/5"
            }`}
          >
            {item.avatar_url ? (
              <Image
                src={item.avatar_url}
                alt={item.username}
                width={24}
                height={24}
                className="h-6 w-6 shrink-0 rounded-full object-cover"
                unoptimized
              />
            ) : (
              <span
                className="grid h-6 w-6 shrink-0 place-items-center rounded-full text-[11px] font-semibold text-white"
                style={{ background: mentionAvatarHue(item.username) }}
                aria-hidden="true"
              >
                {item.username[0]?.toUpperCase()}
              </span>
            )}
            <span className="font-medium">{item.username}</span>
            {item.display_name && <span className="truncate text-muted">· {item.display_name}</span>}
          </button>
        </li>
      ))}
    </ul>
  );
}

const mentionAvatarHue = (s: string) =>
  `oklch(0.55 0.06 ${[...s].reduce((a, c) => a + c.charCodeAt(0), 0) % 360})`;

const DEFAULT_TOOLBAR_STATE = {
  bold: false,
  italic: false,
  strike: false,
  code: false,
  link: false,
  heading1: false,
  heading2: false,
  heading3: false,
  blockquote: false,
  bulletList: false,
  orderedList: false,
  inTable: false,
  canUndo: false,
  canRedo: false,
};

export function RichTextEditor({
  value,
  onChange,
  blogSlug,
  authFetch,
  placeholder,
  notes = [],
  onNotesChange,
  toolbarEnd,
  stickyToolbar = true,
}: RichTextEditorProps) {
  const t = useTranslations("RichTextEditor");
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [uploadError, setUploadError] = useState<string | null>(null);
  // "create": nuova nota inserita al cursore. "edit": modifica dei campi
  // bibliografici di una nota già presente nell'elenco sotto l'editor
  // (idx valorizzato, nessun nuovo marcatore da inserire nel testo).
  const [noteModal, setNoteModal] = useState<{ mode: "create" | "edit"; idx?: number } | null>(null);
  // Snapshot preso una sola volta al primo render: il contenuto iniziale
  // dell'editor non deve rincorrere ogni cambio di `value` (sarebbe l'editor
  // stesso, tramite onUpdate, a farlo cambiare) — solo il caso "arrivato in
  // ritardo" (GET del post ancora in corso al mount) è gestito nell'effect sotto.
  const [initialValue] = useState(value);
  const syncedLateValueRef = useRef(false);

  const editor = useEditor({
    immediatelyRender: false,
    extensions: [
      StarterKit.configure({ link: false }),
      LinkExtension.configure({ openOnClick: false, autolink: true }),
      ImageExtension.extend({ addNodeView: sensitiveImageNodeView }),
      Placeholder.configure({ placeholder: placeholder ?? t("placeholder") }),
      // resizable:false — una larghezza di colonna persistita non è
      // rappresentabile in una tabella Markdown a pipe, che non la prevede.
      // Table.addExtensions() dovrebbe includere già Row/Cell/Header da sé,
      // ma nella pratica lo schema non li registra — aggiunti esplicitamente.
      Table.configure({ resizable: false }),
      TableRow,
      TableHeader,
      TableCell,
      LinkPreviewCard,
      Markdown.configure({ html: false, linkify: true, tightLists: true }),
    ],
    content: initialValue,
    editorProps: {
      attributes: {
        class: "notturni-prose min-h-64 max-w-none text-lg leading-relaxed text-foreground focus:outline-none",
      },
      // Incollare un URL da solo (stile Bluesky, CLAUDE.md #1): il link
      // resta testo libero/cancellabile, la card appare come blocco a sé
      // subito sotto — vedi LinkPreviewCard per il perché di questa scelta.
      handlePaste: (view, event) => {
        const text = event.clipboardData?.getData("text/plain")?.trim();
        if (!text || !/^https?:\/\/\S+$/i.test(text)) return false;

        const { state } = view;
        const { schema } = state;
        const linkMarkType = schema.marks.link;
        const cardNodeType = schema.nodes.linkPreviewCard;
        const paragraphType = schema.nodes.paragraph;
        if (!linkMarkType || !cardNodeType || !paragraphType) return false;

        const linkedText = schema.text(text, [linkMarkType.create({ href: text })]);
        let tr = state.tr.replaceSelectionWith(linkedText, false);
        const $pos = tr.doc.resolve(tr.selection.from);
        const afterParagraph = $pos.after($pos.depth);
        const card = cardNodeType.create({ href: text });
        const emptyParagraph = paragraphType.create();
        // Una riga vuota subito dopo la card, per poter continuare a
        // scrivere senza dover prima creare un nuovo paragrafo a mano.
        tr = tr
          .insert(afterParagraph, card)
          .insert(afterParagraph + card.nodeSize, emptyParagraph);
        tr = tr.setSelection(TextSelection.near(tr.doc.resolve(afterParagraph + card.nodeSize + 1)));
        view.dispatch(tr.scrollIntoView());
        return true;
      },
    },
    onUpdate: ({ editor }) => {
      const markdownStorage = editor.storage as unknown as { markdown: MarkdownStorage };
      onChange(markdownStorage.markdown.getMarkdown());
    },
  });

  const mentionMenu = useMentionAutocomplete(editor, blogSlug, authFetch);

  // Se il contenuto arriva in modo asincrono dopo il mount (es. GET del post
  // ancora in corso al primo render), risincronizza l'editor una volta sola.
  useEffect(() => {
    if (editor && !syncedLateValueRef.current && editor.isEmpty && value && value !== initialValue) {
      editor.commands.setContent(value);
      syncedLateValueRef.current = true;
    }
  }, [editor, value, initialValue]);

  // useEditorState riflette lo stato solo a partire dalla prima transazione
  // successiva alla creazione dell'editor (i suoi listener si agganciano un
  // istante dopo): finché non se ne verifica una, resta al suo snapshot
  // iniziale con editor=null anche se `editor` qui sopra è già pronto. Per
  // questo il fallback qui sotto, non un secondo gate su "!editor" — altrimenti
  // toolbar e contenuto non comparirebbero mai (l'utente non potrebbe produrre
  // la prima transazione senza poter già interagire con l'editor).
  const state = useEditorState({
    editor,
    selector: (ctx) =>
      ctx.editor
        ? {
            bold: ctx.editor.isActive("bold"),
            italic: ctx.editor.isActive("italic"),
            strike: ctx.editor.isActive("strike"),
            code: ctx.editor.isActive("code"),
            link: ctx.editor.isActive("link"),
            heading1: ctx.editor.isActive("heading", { level: 1 }),
            heading2: ctx.editor.isActive("heading", { level: 2 }),
            heading3: ctx.editor.isActive("heading", { level: 3 }),
            blockquote: ctx.editor.isActive("blockquote"),
            bulletList: ctx.editor.isActive("bulletList"),
            orderedList: ctx.editor.isActive("orderedList"),
            inTable: ctx.editor.isActive("table"),
            canUndo: ctx.editor.can().undo(),
            canRedo: ctx.editor.can().redo(),
          }
        : null,
  }) ?? DEFAULT_TOOLBAR_STATE;

  if (!editor) return null;

  function setLink() {
    const previousUrl = editor!.getAttributes("link").href as string | undefined;
    const url = window.prompt(t("linkPrompt"), previousUrl ?? "https://");
    if (url === null) return;
    if (url === "") {
      editor!.chain().focus().extendMarkRange("link").unsetLink().run();
      return;
    }
    editor!.chain().focus().extendMarkRange("link").setLink({ href: url }).run();
  }

  /** Apre il modal "Nota" (mockup 1d), stesso stile di `ContentWarningModal`
   * — al posto del window.prompt bloccante, stesso principio delle pillole
   * ALT/sensibile sulle immagini (SensitiveImageNodeView). La posizione del
   * cursore non serve calcolarla (il modal è un overlay centrato, non un
   * popover ancorato): resta comunque quella corrente della selezione
   * dell'editor, usata solo al salvataggio per inserire il marcatore. */
  function openNoteModal() {
    if (!onNotesChange || !editor) return;
    setNoteModal({ mode: "create" });
  }

  function openEditNoteModal(idx: number) {
    if (!onNotesChange) return;
    setNoteModal({ mode: "edit", idx });
  }

  /** In modalità "create" inserisce al cursore il marcatore `[n](#nota-n)`
   * (un vero nodo link, così sopravvive al round-trip del serializzatore
   * Markdown) e aggiunge la nota all'elenco; in modalità "edit" aggiorna la
   * nota esistente senza toccare il corpo del post. */
  function handleNoteModalSave(value: NoteModalValue) {
    if (!onNotesChange || !noteModal || !editor) return;
    if (value.content.length > MAX_NOTE_LENGTH) {
      setUploadError(t("noteTooLong", { max: MAX_NOTE_LENGTH }));
      return;
    }
    if (noteModal.mode === "edit" && noteModal.idx !== undefined) {
      const idx = noteModal.idx;
      onNotesChange(notes.map((n) => (n.idx === idx ? { ...n, ...value } : n)));
      return;
    }
    const nextIdx = notes.reduce((max, n) => Math.max(max, n.idx), 0) + 1;
    editor
      .chain()
      .focus()
      .insertContent({
        type: "text",
        text: String(nextIdx),
        marks: [{ type: "link", attrs: { href: `#nota-${nextIdx}` } }],
      })
      // il mark link resta "attivo": lo si stacca subito così il testo dopo non ci finisce dentro
      .unsetMark("link")
      .run();
    onNotesChange([...notes, { idx: nextIdx, ...value }]);
  }

  function updateNote(idx: number, content: string) {
    if (!onNotesChange) return;
    onNotesChange(notes.map((n) => (n.idx === idx ? { ...n, content } : n)));
  }

  function removeNote(idx: number) {
    if (!onNotesChange) return;
    onNotesChange(notes.filter((n) => n.idx !== idx));
    // toglie anche i marcatori [idx](#nota-idx) rimasti nel testo
    const ranges: [number, number][] = [];
    editor!.state.doc.descendants((node, pos) => {
      if (
        node.isText &&
        node.marks.some((m) => m.type.name === "link" && m.attrs.href === `#nota-${idx}`)
      ) {
        ranges.push([pos, pos + node.nodeSize]);
      }
    });
    if (ranges.length > 0) {
      const tr = editor!.state.tr;
      ranges.sort((a, b) => b[0] - a[0]).forEach(([from, to]) => tr.delete(from, to));
      editor!.view.dispatch(tr);
    }
  }

  async function handleImagePicked(file: File) {
    if (!blogSlug) return;
    setUploadError(null);
    try {
      const media = await authFetch((token) => api.blogs.uploadMedia(token, blogSlug, file));
      // ALT text e categorie di avviso si impostano dopo l'inserimento,
      // tramite le pillole in sovraimpressione sull'immagine (stile
      // Bluesky, CLAUDE.md #2/#3) — niente più window.prompt bloccante.
      // Un paragrafo vuoto subito dopo permette di continuare a scrivere
      // senza doverlo creare a mano.
      editor!
        .chain()
        .focus()
        .insertContent([
          { type: "image", attrs: { src: media.url, title: media.is_sensitive ? "sensitive" : null } },
          { type: "paragraph" },
        ])
        .run();
    } catch (err) {
      setUploadError(err instanceof ApiClientError ? err.message : t("uploadFailed"));
    }
  }

  return (
    <div>
      {/* Resta visibile durante lo scroll di un contenuto lungo (altrimenti
          bisognerebbe risalire alla cima della pagina per riprendere in
          mano la formattazione). Su mobile i pulsanti scorrono in
          orizzontale invece di andare a capo su più righe, che
          occuperebbero troppo spazio verticale una volta fissati in alto. */}
      <div className={stickyToolbar ? "sticky top-0 z-20 -mx-1 border-b border-border bg-background px-1 pb-3 pt-2" : "mb-3"}>
        {toolbarEnd && (
          <div className="mb-3 flex flex-wrap items-end gap-3">{toolbarEnd}</div>
        )}
        <div className="flex flex-nowrap items-center gap-0.5 overflow-x-auto text-foreground/70">
        <ToolbarButton
          title={t("heading1")}
          active={state.heading1}
          onClick={() => editor.chain().focus().toggleHeading({ level: 1 }).run()}
        >
          <span className="font-bold">H1</span>
        </ToolbarButton>
        <ToolbarButton
          title={t("heading2")}
          active={state.heading2}
          onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()}
        >
          <span className="font-bold">H2</span>
        </ToolbarButton>
        <ToolbarButton
          title={t("heading3")}
          active={state.heading3}
          onClick={() => editor.chain().focus().toggleHeading({ level: 3 }).run()}
        >
          <span className="font-bold">H3</span>
        </ToolbarButton>

        <ToolbarDivider />

        <ToolbarButton title={t("bold")} active={state.bold} onClick={() => editor.chain().focus().toggleBold().run()}>
          <span className="font-bold">B</span>
        </ToolbarButton>
        <ToolbarButton title={t("italic")} active={state.italic} onClick={() => editor.chain().focus().toggleItalic().run()}>
          <span className="italic">I</span>
        </ToolbarButton>
        <ToolbarButton
          title={t("strike")}
          active={state.strike}
          onClick={() => editor.chain().focus().toggleStrike().run()}
        >
          <span className="line-through">S</span>
        </ToolbarButton>
        <ToolbarButton title={t("code")} active={state.code} onClick={() => editor.chain().focus().toggleCode().run()}>
          <span className="font-mono text-xs">{"</>"}</span>
        </ToolbarButton>
        <ToolbarButton title={t("link")} active={state.link} onClick={setLink}>
          <LinkIcon />
        </ToolbarButton>
        {onNotesChange && (
          <ToolbarButton title={t("note")} onClick={openNoteModal}>
            <NoteIcon />
          </ToolbarButton>
        )}

        <ToolbarDivider />

        <ToolbarButton
          title={t("quote")}
          active={state.blockquote}
          onClick={() => editor.chain().focus().toggleBlockquote().run()}
        >
          <QuoteIcon />
        </ToolbarButton>
        <ToolbarButton
          title={t("bulletList")}
          active={state.bulletList}
          onClick={() => editor.chain().focus().toggleBulletList().run()}
        >
          <BulletListIcon />
        </ToolbarButton>
        <ToolbarButton
          title={t("orderedList")}
          active={state.orderedList}
          onClick={() => editor.chain().focus().toggleOrderedList().run()}
        >
          <OrderedListIcon />
        </ToolbarButton>
        {blogSlug && (
          <ToolbarButton title={t("image")} onClick={() => fileInputRef.current?.click()}>
            <ImageIcon />
          </ToolbarButton>
        )}
        <ToolbarButton
          title={t("table")}
          active={state.inTable}
          onClick={() =>
            editor.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run()
          }
        >
          <TableIcon />
        </ToolbarButton>
        {state.inTable && (
          <>
            <ToolbarButton title={t("addColumnTitle")} onClick={() => editor.chain().focus().addColumnAfter().run()}>
              <span className="text-xs">{t("addColumn")}</span>
            </ToolbarButton>
            <ToolbarButton title={t("addRowTitle")} onClick={() => editor.chain().focus().addRowAfter().run()}>
              <span className="text-xs">{t("addRow")}</span>
            </ToolbarButton>
            <ToolbarButton title={t("deleteTableTitle")} onClick={() => editor.chain().focus().deleteTable().run()}>
              <span className="text-xs text-red-700">{t("deleteTable")}</span>
            </ToolbarButton>
          </>
        )}

        <ToolbarDivider />

        <ToolbarButton title={t("undo")} disabled={!state.canUndo} onClick={() => editor.chain().focus().undo().run()}>
          <UndoIcon />
        </ToolbarButton>
        <ToolbarButton title={t("redo")} disabled={!state.canRedo} onClick={() => editor.chain().focus().redo().run()}>
          <RedoIcon />
        </ToolbarButton>
        </div>
      </div>

      {blogSlug && (
        <input
          ref={fileInputRef}
          type="file"
          accept="image/png,image/jpeg,image/webp,image/gif"
          className="hidden"
          onChange={(e) => {
            const file = e.target.files?.[0];
            if (file) void handleImagePicked(file);
            e.target.value = "";
          }}
        />
      )}

      {uploadError && <p className="mb-2 text-xs text-red-700">{uploadError}</p>}

      <EditorContent editor={editor} />
      {mentionMenu}

      {noteModal && onNotesChange && (
        <NoteModal
          initial={
            noteModal.mode === "edit" ? notes.find((n) => n.idx === noteModal.idx) : undefined
          }
          onSave={handleNoteModalSave}
          onClose={() => setNoteModal(null)}
        />
      )}

      {onNotesChange && notes.length > 0 && (
        <div className="mt-8 border-t border-border pt-4">
          <p className="mb-2 text-xs uppercase tracking-wide text-muted">{t("footnotesTitle")}</p>
          <ul className="space-y-2">
            {[...notes]
              .sort((a, b) => a.idx - b.idx)
              .map((note) => {
                const hasDetails = Boolean(
                  note.title || note.author || note.kind || note.source || note.issued || note.isbn || note.doi || note.url || note.page
                );
                return (
                  <li key={note.idx} className="flex items-start gap-2">
                    <span className="mt-2 w-5 shrink-0 text-right text-xs text-muted">{note.idx}.</span>
                    <textarea
                      value={note.content}
                      maxLength={MAX_NOTE_LENGTH}
                      rows={2}
                      onChange={(e) => updateNote(note.idx, e.target.value)}
                      className="flex-1 rounded-md border border-border bg-background px-2 py-1.5 text-sm text-foreground"
                    />
                    <div className="mt-1 flex shrink-0 flex-col items-end gap-1 text-xs">
                      <button
                        type="button"
                        onClick={() => openEditNoteModal(note.idx)}
                        className={hasDetails ? "font-medium text-primary" : "text-muted hover:text-foreground"}
                        title={hasDetails ? t("noteHasDetails") : undefined}
                      >
                        {t("editDetails")}
                        {hasDetails ? " ●" : ""}
                      </button>
                      <button
                        type="button"
                        onClick={() => removeNote(note.idx)}
                        className="text-muted hover:text-foreground"
                      >
                        {t("remove")}
                      </button>
                    </div>
                  </li>
                );
              })}
          </ul>
          <p className="mt-2 text-xs text-muted">{t("footnoteHint")}</p>
        </div>
      )}
    </div>
  );
}
