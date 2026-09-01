"use client";

import ImageExtension from "@tiptap/extension-image";
import LinkExtension from "@tiptap/extension-link";
import Placeholder from "@tiptap/extension-placeholder";
import { Table } from "@tiptap/extension-table";
import TableCell from "@tiptap/extension-table-cell";
import TableHeader from "@tiptap/extension-table-header";
import TableRow from "@tiptap/extension-table-row";
import type { Editor } from "@tiptap/react";
import { EditorContent, useEditor, useEditorState } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
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

interface RichTextEditorProps {
  /** Contenuto iniziale in Markdown (il backend salva/legge solo Markdown). */
  value: string;
  onChange: (markdown: string) => void;
  /** Slug del blog: serve per caricare le immagini incorporate su S3/MinIO. */
  blogSlug: string;
  authFetch: <T>(fn: (token: string) => Promise<T>) => Promise<T>;
  placeholder?: string;
  /** Note a piè di pagina del post (todo/EDITOR.md). */
  notes: PostNote[];
  onNotesChange: (notes: PostNote[]) => void;
}

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
      className={`inline-flex h-9 w-9 items-center justify-center rounded-full text-base transition disabled:opacity-30 disabled:cursor-not-allowed ${
        active ? "bg-primary/10 text-primary" : "text-foreground/70 hover:bg-foreground/5 hover:text-foreground"
      }`}
    >
      {children}
    </button>
  );
}

function ToolbarDivider() {
  return <span className="mx-1.5 h-6 w-px bg-border" />;
}

interface MentionCandidate {
  username: string;
  display_name: string | null;
}

// La `@` deve essere a inizio riga o preceduta da uno spazio; poi 0..32
// caratteri del formato username (vedi app/domain/usernames.py).
const MENTION_TRIGGER_RE = /(?:^|\s)@([a-z0-9_-]{0,32})$/;

/** Autocomplete delle @menzioni nell'editor (todo/EDITOR.md): rileva la
 * digitazione di `@parola`, propone gli utenti del blog e, alla selezione,
 * inserisce `@username ` come testo semplice (il Markdown resta pulito, il
 * link si forma al rendering — vedi src/lib/markdown.ts). Se il blog ha le
 * menzioni disattivate l'endpoint non restituisce nulla e il menu non appare. */
function useMentionAutocomplete(
  editor: Editor | null,
  blogSlug: string,
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
    if (!editor) return;
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
  }, [editor, close]);

  const query = anchor?.query;
  const open = anchor !== null;
  useEffect(() => {
    if (!open) return;
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
      className="max-h-56 w-64 overflow-auto rounded-lg border border-border bg-background py-1 text-sm shadow-lg"
    >
      {items.map((item, i) => (
        <li key={item.username}>
          <button
            type="button"
            onMouseDown={(e) => {
              e.preventDefault();
              applyMention(item);
            }}
            className={`flex w-full items-center gap-1.5 px-3 py-1.5 text-left ${
              i === index ? "bg-primary/10 text-foreground" : "text-foreground/80 hover:bg-foreground/5"
            }`}
          >
            <span className="text-muted">@</span>
            <span className="font-medium">{item.username}</span>
            {item.display_name && <span className="truncate text-muted">· {item.display_name}</span>}
          </button>
        </li>
      ))}
    </ul>
  );
}

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
  notes,
  onNotesChange,
}: RichTextEditorProps) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [uploadError, setUploadError] = useState<string | null>(null);
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
      ImageExtension,
      Placeholder.configure({ placeholder: placeholder ?? "Scrivi qualcosa..." }),
      // resizable:false — una larghezza di colonna persistita non è
      // rappresentabile in una tabella Markdown a pipe, che non la prevede.
      // Table.addExtensions() dovrebbe includere già Row/Cell/Header da sé,
      // ma nella pratica lo schema non li registra — aggiunti esplicitamente.
      Table.configure({ resizable: false }),
      TableRow,
      TableHeader,
      TableCell,
      Markdown.configure({ html: false, linkify: true, tightLists: true }),
    ],
    content: initialValue,
    editorProps: {
      attributes: {
        class: "notturni-prose min-h-64 max-w-none text-lg leading-relaxed text-foreground focus:outline-none",
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
    const url = window.prompt("URL del link", previousUrl ?? "https://");
    if (url === null) return;
    if (url === "") {
      editor!.chain().focus().extendMarkRange("link").unsetLink().run();
      return;
    }
    editor!.chain().focus().extendMarkRange("link").setLink({ href: url }).run();
  }

  /** Inserisce al cursore il marcatore `[n](#nota-n)` (un vero nodo link, così
   * sopravvive al round-trip del serializzatore Markdown) e aggiunge la nota
   * all'elenco. */
  function insertNote() {
    const text = window.prompt("Testo della nota");
    if (text === null) return;
    const trimmed = text.trim();
    if (!trimmed) return;
    if (trimmed.length > MAX_NOTE_LENGTH) {
      setUploadError(`La nota supera i ${MAX_NOTE_LENGTH} caratteri.`);
      return;
    }
    const nextIdx = notes.reduce((max, n) => Math.max(max, n.idx), 0) + 1;
    editor!
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
    onNotesChange([...notes, { idx: nextIdx, content: trimmed }]);
  }

  function updateNote(idx: number, content: string) {
    onNotesChange(notes.map((n) => (n.idx === idx ? { ...n, content } : n)));
  }

  function removeNote(idx: number) {
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
    setUploadError(null);
    try {
      const media = await authFetch((token) => api.blogs.uploadMedia(token, blogSlug, file));
      const alt = window.prompt("Testo alternativo dell'immagine (per l'accessibilità)", "") ?? "";
      editor!
        .chain()
        .focus()
        .setImage({ src: media.url, alt: alt || undefined, title: media.is_sensitive ? "sensitive" : undefined })
        .run();
      if (media.is_sensitive) {
        setUploadError(
          "L'immagine è stata segnalata come possibile contenuto sensibile: verrà mostrata sfocata ai lettori, cliccabile per rivelarla."
        );
      }
    } catch (err) {
      setUploadError(err instanceof ApiClientError ? err.message : "Caricamento immagine non riuscito.");
    }
  }

  return (
    <div>
      {/* Barra flottante, senza contenitore: solo un filo di spazio la separa
          dal testo, niente riquadro o bordo a delimitarla. */}
      <div className="mb-3 flex flex-wrap items-center gap-1 text-foreground/70">
        <ToolbarButton
          title="Titolo 1"
          active={state.heading1}
          onClick={() => editor.chain().focus().toggleHeading({ level: 1 }).run()}
        >
          <span className="font-bold">H1</span>
        </ToolbarButton>
        <ToolbarButton
          title="Titolo 2"
          active={state.heading2}
          onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()}
        >
          <span className="font-bold">H2</span>
        </ToolbarButton>
        <ToolbarButton
          title="Titolo 3"
          active={state.heading3}
          onClick={() => editor.chain().focus().toggleHeading({ level: 3 }).run()}
        >
          <span className="font-bold">H3</span>
        </ToolbarButton>

        <ToolbarDivider />

        <ToolbarButton title="Grassetto" active={state.bold} onClick={() => editor.chain().focus().toggleBold().run()}>
          <span className="font-bold">B</span>
        </ToolbarButton>
        <ToolbarButton title="Corsivo" active={state.italic} onClick={() => editor.chain().focus().toggleItalic().run()}>
          <span className="italic">I</span>
        </ToolbarButton>
        <ToolbarButton
          title="Barrato"
          active={state.strike}
          onClick={() => editor.chain().focus().toggleStrike().run()}
        >
          <span className="line-through">S</span>
        </ToolbarButton>
        <ToolbarButton title="Codice" active={state.code} onClick={() => editor.chain().focus().toggleCode().run()}>
          <span className="font-mono text-xs">{"</>"}</span>
        </ToolbarButton>
        <ToolbarButton title="Link" active={state.link} onClick={setLink}>
          <LinkIcon />
        </ToolbarButton>
        <ToolbarButton title="Nota a piè di pagina" onClick={insertNote}>
          <NoteIcon />
        </ToolbarButton>

        <ToolbarDivider />

        <ToolbarButton
          title="Citazione"
          active={state.blockquote}
          onClick={() => editor.chain().focus().toggleBlockquote().run()}
        >
          <QuoteIcon />
        </ToolbarButton>
        <ToolbarButton
          title="Elenco puntato"
          active={state.bulletList}
          onClick={() => editor.chain().focus().toggleBulletList().run()}
        >
          <BulletListIcon />
        </ToolbarButton>
        <ToolbarButton
          title="Elenco numerato"
          active={state.orderedList}
          onClick={() => editor.chain().focus().toggleOrderedList().run()}
        >
          <OrderedListIcon />
        </ToolbarButton>
        <ToolbarButton title="Immagine" onClick={() => fileInputRef.current?.click()}>
          <ImageIcon />
        </ToolbarButton>
        <ToolbarButton
          title="Tabella"
          active={state.inTable}
          onClick={() =>
            editor.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run()
          }
        >
          <TableIcon />
        </ToolbarButton>
        {state.inTable && (
          <>
            <ToolbarButton title="Aggiungi colonna" onClick={() => editor.chain().focus().addColumnAfter().run()}>
              <span className="text-xs">+col</span>
            </ToolbarButton>
            <ToolbarButton title="Aggiungi riga" onClick={() => editor.chain().focus().addRowAfter().run()}>
              <span className="text-xs">+riga</span>
            </ToolbarButton>
            <ToolbarButton title="Elimina tabella" onClick={() => editor.chain().focus().deleteTable().run()}>
              <span className="text-xs text-red-700">✕tab</span>
            </ToolbarButton>
          </>
        )}

        <ToolbarDivider />

        <ToolbarButton title="Annulla" disabled={!state.canUndo} onClick={() => editor.chain().focus().undo().run()}>
          <UndoIcon />
        </ToolbarButton>
        <ToolbarButton title="Ripeti" disabled={!state.canRedo} onClick={() => editor.chain().focus().redo().run()}>
          <RedoIcon />
        </ToolbarButton>
      </div>

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

      {uploadError && <p className="mb-2 text-xs text-red-700">{uploadError}</p>}

      <EditorContent editor={editor} />
      {mentionMenu}

      {notes.length > 0 && (
        <div className="mt-8 border-t border-border pt-4">
          <p className="mb-2 text-xs uppercase tracking-wide text-muted">Note a piè di pagina</p>
          <ul className="space-y-2">
            {[...notes]
              .sort((a, b) => a.idx - b.idx)
              .map((note) => (
                <li key={note.idx} className="flex items-start gap-2">
                  <span className="mt-2 w-5 shrink-0 text-right text-xs text-muted">{note.idx}.</span>
                  <textarea
                    value={note.content}
                    maxLength={MAX_NOTE_LENGTH}
                    rows={2}
                    onChange={(e) => updateNote(note.idx, e.target.value)}
                    className="flex-1 rounded-md border border-border bg-background px-2 py-1.5 text-sm text-foreground"
                  />
                  <button
                    type="button"
                    onClick={() => removeNote(note.idx)}
                    className="mt-1 text-xs text-muted hover:text-foreground"
                  >
                    Rimuovi
                  </button>
                </li>
              ))}
          </ul>
          <p className="mt-2 text-xs text-muted">
            Il riferimento nel testo è il numero cliccabile inserito col pulsante «Nota».
          </p>
        </div>
      )}
    </div>
  );
}
