import { fireEvent, screen } from "@testing-library/react";
import { Editor } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import type { NodeViewProps } from "@tiptap/react";
import { Markdown, type MarkdownStorage } from "tiptap-markdown";
import { describe, expect, it, vi } from "vitest";

import type { GalleryItem } from "@/lib/gallery";
import { renderWithIntl } from "@/test/render";

import { EditorGalleryExtension, EditorImageExtension } from "./RichTextEditor";
import { GalleryAddContext, GalleryNodeView } from "./GalleryNodeView";

const item = (n: number, alt = `alt ${n}`): GalleryItem => ({
  url: `/m/${n}.png`,
  alt,
  sensitive: false,
  categories: [],
});

function setup(items: GalleryItem[], opts: { addImages?: (pos: number) => void } = {}) {
  const updateAttributes = vi.fn();
  const deleteNode = vi.fn();
  const getPos = vi.fn(() => 7);
  const props = {
    node: { attrs: { items } },
    updateAttributes,
    deleteNode,
    getPos,
    selected: false,
  } as unknown as NodeViewProps;
  const addImages = opts.addImages ?? null;
  renderWithIntl(
    <GalleryAddContext.Provider value={addImages}>
      <GalleryNodeView {...props} />
    </GalleryAddContext.Provider>,
  );
  return { updateAttributes, deleteNode, getPos, addImages };
}

describe("GalleryNodeView", () => {
  it("move down swaps the item with the next one", () => {
    const { updateAttributes } = setup([item(1), item(2), item(3)]);
    fireEvent.click(screen.getAllByRole("button", { name: "Move image down" })[0]);
    expect(updateAttributes).toHaveBeenCalledWith({ items: [item(2), item(1), item(3)] });
  });

  it("move up swaps the item with the previous one", () => {
    const { updateAttributes } = setup([item(1), item(2), item(3)]);
    fireEvent.click(screen.getAllByRole("button", { name: "Move image up" })[2]);
    expect(updateAttributes).toHaveBeenCalledWith({ items: [item(1), item(3), item(2)] });
  });

  it("first item cannot move up and last cannot move down", () => {
    setup([item(1), item(2)]);
    expect(screen.getAllByRole("button", { name: "Move image up" })[0]).toBeDisabled();
    expect(screen.getAllByRole("button", { name: "Move image down" })[1]).toBeDisabled();
  });

  it("removing one of several images keeps the gallery", () => {
    const { updateAttributes, deleteNode } = setup([item(1), item(2)]);
    fireEvent.click(screen.getAllByRole("button", { name: "Remove image" })[0]);
    expect(updateAttributes).toHaveBeenCalledWith({ items: [item(2)] });
    expect(deleteNode).not.toHaveBeenCalled();
  });

  it("removing the last image deletes the node", () => {
    const { deleteNode, updateAttributes } = setup([item(1)]);
    fireEvent.click(screen.getByRole("button", { name: "Remove image" }));
    expect(deleteNode).toHaveBeenCalledTimes(1);
    expect(updateAttributes).not.toHaveBeenCalled();
  });

  it("alt edit updates the attribute on blur", () => {
    const { updateAttributes } = setup([item(1, ""), item(2)]);
    const input = screen.getAllByRole("textbox", { name: "Testo alternativo" })[0];
    fireEvent.change(input, { target: { value: "  un gatto  " } });
    fireEvent.blur(input);
    expect(updateAttributes).toHaveBeenCalledWith({
      items: [{ ...item(1, ""), alt: "un gatto" }, item(2)],
    });
  });

  it("blur without change does not write", () => {
    const { updateAttributes } = setup([item(1, "x")]);
    fireEvent.blur(screen.getByRole("textbox", { name: "Testo alternativo" }));
    expect(updateAttributes).not.toHaveBeenCalled();
  });

  it("add action is shown below 10 images and opens the picker at the node position", () => {
    const addImages = vi.fn();
    setup([item(1), item(2)], { addImages });
    fireEvent.click(screen.getByRole("button", { name: "+ Image" }));
    expect(addImages).toHaveBeenCalledWith(7);
  });

  it("add action is hidden at 10 images", () => {
    setup(Array.from({ length: 10 }, (_, i) => item(i)), { addImages: vi.fn() });
    expect(screen.queryByRole("button", { name: "+ Image" })).toBeNull();
  });

  it("add action is hidden without an editor context", () => {
    setup([item(1), item(2)]);
    expect(screen.queryByRole("button", { name: "+ Image" })).toBeNull();
  });
});

function roundTrip(content: unknown): string {
  const editor = new Editor({
    extensions: [
      StarterKit,
      EditorImageExtension,
      EditorGalleryExtension,
      Markdown.configure({ html: false, linkify: true, tightLists: true }),
    ],
    content: content as never,
  });
  const out = (editor.storage as unknown as { markdown: MarkdownStorage }).markdown.getMarkdown();
  editor.destroy();
  return out;
}

const para = (text: string) => ({ type: "paragraph", content: [{ type: "text", text }] });
const image = (src: string) => ({ type: "image", attrs: { src, alt: null, title: null } });

describe("editor Markdown output", () => {
  it("a paragraph after an image is not glued to it", () => {
    expect(roundTrip({ type: "doc", content: [image("/a.png"), para("after")] })).toBe(
      "![](/a.png)\n\nafter",
    );
  });

  it("an image between paragraphs keeps a blank line on both sides", () => {
    expect(roundTrip({ type: "doc", content: [para("before"), image("/a.png"), para("after")] })).toBe(
      "before\n\n![](/a.png)\n\nafter",
    );
  });

  it("a gallery after an image starts on its own block", () => {
    const md = roundTrip({
      type: "doc",
      content: [
        image("/a.png"),
        { type: "gallery", attrs: { items: [item(1), item(2)] } },
      ],
    });
    expect(md).toBe("![](/a.png)\n\n```gallery\n![alt 1](/m/1.png)\n![alt 2](/m/2.png)\n```");
  });
});
