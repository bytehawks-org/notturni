import CodeBlock from "@tiptap/extension-code-block";
import { Heading } from "@tiptap/extension-heading";
import { Paragraph } from "@tiptap/extension-paragraph";
import { TextAlign } from "@tiptap/extension-text-align";
import { Underline } from "@tiptap/extension-underline";

import { codeBlockMarkdownPlugin, textAlignMarkdownPlugin, underlineMarkdownPlugin } from "@/lib/markdown-format-extensions";

/** Sottolineato: StarterKit lo include già nello schema (`underline: false`
 * lo disattiva lì per rimpiazzarlo con questo `.extend()`, stesso pattern di
 * `ImageExtension.extend()` in RichTextEditor.tsx), ma `tiptap-markdown`
 * 0.9.0 non ha una serializzazione predefinita per questo mark: senza
 * l'`addStorage` qui sotto il testo sottolineato spariva silenziosamente al
 * salvataggio (il mark restava nello stato dell'editor ma mai nel Markdown
 * salvato). Sintassi `++testo++`, vedi lib/markdown-format-extensions.ts. */
export const UnderlineMark = Underline.extend({
  addStorage() {
    return {
      markdown: {
        serialize: {
          open: "++",
          close: "++",
          expelEnclosingWhitespace: true,
        },
        parse: {
          setup: underlineMarkdownPlugin,
        },
      },
    };
  },
});

/** Allineamento del testo (sinistra/centro/destra) su paragrafi e titoli.
 * L'attributo `textAlign` è iniettato da `TextAlignExtension` sotto (stesso
 * meccanismo del pacchetto ufficiale: `addGlobalAttributes`, che si occupa
 * già da sé di leggere/scrivere `style="text-align: ..."` in HTML) — qui
 * serve solo estendere `Paragraph`/`Heading` per insegnare a
 * `tiptap-markdown` a *salvare* quell'attributo, dato che
 * `defaultMarkdownSerializer` non lo conosce. Marcatore `{: .center}` a fine
 * blocco, vedi lib/markdown-format-extensions.ts. */
export const ParagraphNode = Paragraph.extend({
  addStorage() {
    return {
      markdown: {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any -- MarkdownSerializerState non è tipizzato da tiptap-markdown
        serialize: (state: any, node: any) => {
          state.renderInline(node);
          if (node.attrs.textAlign && node.attrs.textAlign !== "left") {
            state.ensureNewLine();
            state.write(`{: .${node.attrs.textAlign}}`);
          }
          state.closeBlock(node);
        },
        parse: {
          setup: textAlignMarkdownPlugin,
        },
      },
    };
  },
});

/** Blocco di codice con linguaggio ed evidenziazione sintassi (blocco
 * "evidenziazione sintassi"): estende il `CodeBlock` ufficiale di TipTap
 * invece di lasciare quello che `tiptap-markdown` inietta di default —
 * stesso principio di `HeadingNode`/`ParagraphNode`/`UnderlineMark` sopra,
 * un'estensione con lo stesso nome registrata più avanti nell'elenco di
 * `RichTextEditor.tsx` vince sulla versione di default. Il linguaggio è lo
 * standard ```` ```lang ````; la numerazione delle righe (a discrezione di
 * chi scrive) è un'estensione non-CommonMark, seconda parola dell'info
 * string (```` ```python line-numbers ````) — stesso `codeBlockMarkdownPlugin`
 * usato dal renderer pubblico (lib/markdown.ts) per riconoscerla in HTML
 * (`data-line-numbers` sul `<pre>`), letta qui in `parseHTML`. */
export const CodeBlockNode = CodeBlock.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      lineNumbers: {
        default: false,
        parseHTML: (element: HTMLElement) => element.hasAttribute("data-line-numbers"),
        renderHTML: (attributes: { lineNumbers?: boolean }) =>
          attributes.lineNumbers ? { "data-line-numbers": "" } : {},
      },
    };
  },
  addStorage() {
    return {
      markdown: {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any -- MarkdownSerializerState non è tipizzato da tiptap-markdown
        serialize: (state: any, node: any) => {
          const info = [node.attrs.language || "", node.attrs.lineNumbers ? "line-numbers" : ""]
            .filter(Boolean)
            .join(" ");
          state.write("```" + info + "\n");
          state.text(node.textContent, false);
          state.ensureNewLine();
          state.write("```");
          state.closeBlock(node);
        },
        parse: {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any -- MarkdownIt non tipizzato da tiptap-markdown qui
          setup: (markdownit: any) => {
            markdownit.set({ languageClassPrefix: "language-", langPrefix: "language-" });
            codeBlockMarkdownPlugin(markdownit);
          },
          // Stessa pulizia già presente nel CodeBlock di default di
          // tiptap-markdown: un ritorno a capo prima di `</code></pre>`
          // finirebbe altrimenti dentro il testo del blocco (riga vuota in
          // più ad ogni giro editor → markdown → editor).
          updateDOM: (element: HTMLElement) => {
            element.innerHTML = element.innerHTML.replace(/\n<\/code><\/pre>/g, "</code></pre>");
          },
        },
      },
    };
  },
});

export const HeadingNode = Heading.extend({
  addStorage() {
    return {
      markdown: {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any -- MarkdownSerializerState non è tipizzato da tiptap-markdown
        serialize: (state: any, node: any) => {
          state.write(state.repeat("#", node.attrs.level) + " ");
          state.renderInline(node, false);
          if (node.attrs.textAlign && node.attrs.textAlign !== "left") {
            state.write(` {: .${node.attrs.textAlign}}`);
          }
          state.closeBlock(node);
        },
        parse: {
          setup: textAlignMarkdownPlugin,
        },
      },
    };
  },
});

export const TextAlignExtension = TextAlign.configure({
  types: ["heading", "paragraph"],
});
