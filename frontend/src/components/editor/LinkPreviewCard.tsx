"use client";

import { Node, mergeAttributes } from "@tiptap/core";
import { NodeViewWrapper, ReactNodeViewRenderer, type NodeViewProps } from "@tiptap/react";
import { Plugin, PluginKey, type Transaction } from "@tiptap/pm/state";
import { useEffect, useState } from "react";

import { ApiClientError, api } from "@/lib/api";
import { CloseIcon } from "./icons";

/** Anteprima di un link, stile Bluesky (CLAUDE.md #1): un nodo a sé, non
 * legato al testo del link incollato — così quest'ultimo resta liberamente
 * modificabile/cancellabile (anche del tutto, lasciando solo la card) senza
 * intaccare la card, che vive come blocco indipendente subito sotto.
 *
 * Nessun dato dell'anteprima (titolo/descrizione/immagine) è salvato nel
 * documento: solo l'URL. Il titolo/descrizione/immagine si (ri)caricano ad
 * ogni apertura dell'editor tramite GET /api/v1/link-preview, così restano
 * sempre aggiornati — stesso principio "niente snapshot" già scelto per il
 * nome pubblico dell'autore (CLAUDE.md #1). Nel Markdown il nodo è salvato
 * come un link con `title="card"` (`[url](url "card")`, stessa convenzione
 * di `sensitive` sulle immagini) — vedi il plugin ProseMirror più sotto per
 * come un link con quel marcatore, caricato da un post esistente, si
 * "promuove" di nuovo a card nell'editor. */
function LinkPreviewCardView({ node, deleteNode }: NodeViewProps) {
  const href = node.attrs.href as string;
  const [preview, setPreview] = useState<{ title: string | null; description: string | null; image: string | null } | null>(
    null
  );
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api.linkPreview
      .get(href)
      .then((p) => {
        if (!cancelled) setPreview(p);
      })
      .catch((err) => {
        if (!cancelled) {
          setFailed(true);
          if (!(err instanceof ApiClientError)) throw err;
        }
      });
    return () => {
      cancelled = true;
    };
  }, [href]);

  let hostname = href;
  try {
    hostname = new URL(href).hostname;
  } catch {
    // href non valido/relativo: mostriamo il testo così com'è.
  }

  return (
    <NodeViewWrapper as="div" contentEditable={false} className="relative my-3 max-w-md">
      <a
        href={href}
        target="_blank"
        rel="noopener noreferrer nofollow"
        onClick={(e) => e.preventDefault()}
        className="block overflow-hidden rounded-xl border border-border transition hover:border-primary/50"
      >
        {preview?.image && (
          // eslint-disable-next-line @next/next/no-img-element -- URL esterno arbitrario
          <img src={preview.image} alt="" className="h-40 w-full object-cover" />
        )}
        <div className="p-3">
          <p className="text-xs text-muted">{hostname}</p>
          <p className="mt-0.5 line-clamp-2 text-sm font-medium text-foreground">
            {preview?.title || (failed ? href : "Caricamento anteprima…")}
          </p>
          {preview?.description && (
            <p className="mt-1 line-clamp-2 text-xs text-muted">{preview.description}</p>
          )}
        </div>
      </a>
      <button
        type="button"
        onClick={() => deleteNode()}
        title="Rimuovi anteprima"
        className="absolute right-2 top-2 flex h-7 w-7 items-center justify-center rounded-full bg-foreground/70 text-background"
      >
        <CloseIcon />
      </button>
    </NodeViewWrapper>
  );
}

export const LinkPreviewCard = Node.create({
  name: "linkPreviewCard",
  group: "block",
  atom: true,
  draggable: true,
  selectable: true,

  addAttributes() {
    return {
      href: { default: null },
    };
  },

  parseHTML() {
    return [{ tag: "div[data-link-preview-card]" }];
  },

  renderHTML({ HTMLAttributes }) {
    return ["div", mergeAttributes(HTMLAttributes, { "data-link-preview-card": "" })];
  },

  addNodeView() {
    return ReactNodeViewRenderer(LinkPreviewCardView);
  },

  addStorage() {
    return {
      markdown: {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any -- MarkdownSerializerState non è tipizzato da tiptap-markdown
        serialize: (state: any, node: any) => {
          state.write(`[${node.attrs.href}](${node.attrs.href} "card")`);
          state.closeBlock(node);
        },
      },
    };
  },

  // Un post/pagina esistente ha la card salvata come un normale link con
  // title="card" (markdown-it non sa nulla di questo nodo custom, la
  // analizza come mark `link`) — questo plugin la "promuove" a card non
  // appena il documento viene caricato nell'editor, sostituendo il
  // paragrafo che contiene solo quel link con il nodo linkPreviewCard.
  addProseMirrorPlugins() {
    const nodeName = this.name;
    return [
      new Plugin({
        key: new PluginKey("linkPreviewCardUpgrade"),
        appendTransaction(transactions, _oldState, newState) {
          if (!transactions.some((t) => t.docChanged)) return null;
          const cardType = newState.schema.nodes[nodeName];
          if (!cardType) return null;

          let tr: Transaction | null = null;
          newState.doc.descendants((node, pos) => {
            if (tr) return false;
            if (node.type.name !== "paragraph" || node.childCount !== 1) return true;
            const child = node.firstChild;
            if (!child || !child.isText || !child.text) return true;
            const linkMark = child.marks.find((m) => m.type.name === "link");
            if (!linkMark || linkMark.attrs.title !== "card") return true;

            tr = newState.tr.replaceWith(pos, pos + node.nodeSize, cardType.create({ href: linkMark.attrs.href }));
            return false;
          });
          return tr;
        },
      }),
    ];
  },
});
