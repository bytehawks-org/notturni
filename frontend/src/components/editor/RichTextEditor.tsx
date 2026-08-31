"use client";

import ImageExtension from "@tiptap/extension-image";
import LinkExtension from "@tiptap/extension-link";
import Placeholder from "@tiptap/extension-placeholder";
import { EditorContent, useEditor, useEditorState } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { useEffect, useRef, useState } from "react";
import { Markdown, type MarkdownStorage } from "tiptap-markdown";

import { ApiClientError, api } from "@/lib/api";
import {
  BulletListIcon,
  ImageIcon,
  LinkIcon,
  OrderedListIcon,
  QuoteIcon,
  RedoIcon,
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

const DEFAULT_TOOLBAR_STATE = {
  bold: false,
  italic: false,
  strike: false,
  code: false,
  link: false,
  blockquote: false,
  bulletList: false,
  orderedList: false,
  canUndo: false,
  canRedo: false,
};

export function RichTextEditor({ value, onChange, blogSlug, authFetch, placeholder }: RichTextEditorProps) {
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
      Markdown.configure({ html: false, linkify: true, tightLists: true }),
    ],
    content: initialValue,
    editorProps: {
      attributes: {
        class: "prose-editor min-h-64 max-w-none text-lg leading-relaxed text-foreground focus:outline-none",
      },
    },
    onUpdate: ({ editor }) => {
      const markdownStorage = editor.storage as unknown as { markdown: MarkdownStorage };
      onChange(markdownStorage.markdown.getMarkdown());
    },
  });

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
            blockquote: ctx.editor.isActive("blockquote"),
            bulletList: ctx.editor.isActive("bulletList"),
            orderedList: ctx.editor.isActive("orderedList"),
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

  async function handleImagePicked(file: File) {
    setUploadError(null);
    try {
      const { url } = await authFetch((token) => api.blogs.uploadMedia(token, blogSlug, file));
      editor!.chain().focus().setImage({ src: url }).run();
    } catch (err) {
      setUploadError(err instanceof ApiClientError ? err.message : "Caricamento immagine non riuscito.");
    }
  }

  return (
    <div>
      {/* Barra flottante, senza contenitore: solo un filo di spazio la separa
          dal testo, niente riquadro o bordo a delimitarla. */}
      <div className="mb-3 flex flex-wrap items-center gap-1 text-foreground/70">
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
    </div>
  );
}
