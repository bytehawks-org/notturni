"use client";

import dynamic from "next/dynamic";

import type { RichTextEditorProps } from "./RichTextEditor";

// TipTap (StarterKit + estensioni tabella/immagine/link/markdown) è il
// bundle più pesante dell'editor: caricato solo quando l'utente apre
// davvero una pagina con RichTextEditor (dashboard/admin, mai una pagina
// pubblica) invece di finire nel chunk della route caricato subito.
const LazyRichTextEditor = dynamic(() => import("./RichTextEditor").then((m) => m.RichTextEditor), {
  ssr: false,
  loading: () => <div className="h-64 animate-pulse rounded-xl border border-border bg-surface" />,
});

export function RichTextEditor(props: RichTextEditorProps) {
  return <LazyRichTextEditor {...props} />;
}
