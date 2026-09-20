import { Heading } from "@tiptap/extension-heading";
import { Paragraph } from "@tiptap/extension-paragraph";
import { TextAlign } from "@tiptap/extension-text-align";
import { Underline } from "@tiptap/extension-underline";

import { textAlignMarkdownPlugin, underlineMarkdownPlugin } from "@/lib/markdown-format-extensions";

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
