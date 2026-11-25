"use client";

import { NodeViewWrapper, ReactNodeViewRenderer, type NodeViewProps } from "@tiptap/react";
import { useTranslations } from "next-intl";
import { createContext, useContext, useId } from "react";

import { GALLERY_MAX, galleryLayout, type GalleryItem } from "@/lib/gallery";

import { clampGalleryItems } from "./GalleryExtension";

/** Lets a gallery node ask the editor to open the picker for its own position.
 * Provided by RichTextEditor; null outside the editor (no "add" action then). */
export const GalleryAddContext = createContext<((pos: number) => void) | null>(null);

/** Node view of the gallery block: thumbnail grid (same layout classes as the
 * public page), then one row per image with alt text, move up/down and remove.
 * Removing the last image deletes the node; a gallery of 1 stays a gallery.
 * Alt text is committed on blur, so each edit is one undo step. */
export function GalleryNodeView({ node, updateAttributes, deleteNode, getPos, selected }: NodeViewProps) {
  const t = useTranslations("Gallery");
  const tEditor = useTranslations("RichTextEditor");
  const addImages = useContext(GalleryAddContext);
  const idBase = useId();

  const items = clampGalleryItems(node.attrs.items);
  const layout = galleryLayout(items.length);
  const canAdd = addImages !== null && items.length < GALLERY_MAX;

  function commit(next: GalleryItem[]) {
    if (next.length === 0) {
      deleteNode();
      return;
    }
    updateAttributes({ items: next });
  }

  function remove(index: number) {
    commit(items.filter((_, i) => i !== index));
  }

  function move(index: number, delta: -1 | 1) {
    const target = index + delta;
    if (target < 0 || target >= items.length) return;
    const next = [...items];
    [next[index], next[target]] = [next[target], next[index]];
    updateAttributes({ items: next });
  }

  function setAlt(index: number, alt: string) {
    if (items[index].alt === alt) return;
    updateAttributes({ items: items.map((item, i) => (i === index ? { ...item, alt } : item)) });
  }

  function requestAdd() {
    const pos = getPos();
    if (addImages && pos !== undefined) addImages(pos);
  }

  const buttonClass =
    "flex h-11 w-11 shrink-0 items-center justify-center rounded-md border border-border bg-background text-lg text-foreground disabled:opacity-40 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary";

  return (
    <NodeViewWrapper
      as="div"
      role="group"
      aria-label={t("editorBlockLabel")}
      className={`my-3 rounded-xl border border-border bg-surface p-3 ${selected ? "outline outline-2 outline-primary" : ""}`}
    >
      <div className={`post-gallery post-gallery--${layout.modifier}`} aria-hidden="true">
        {items.slice(0, layout.visible).map((item, i) => (
          <div key={`${i}:${item.url}`} className="post-gallery__tile">
            {/* eslint-disable-next-line @next/next/no-img-element -- URL storage esterno, non ottimizzabile da next/image */}
            <img src={item.url} alt="" />
          </div>
        ))}
      </div>

      <ol className="mt-3 flex flex-col gap-2">
        {items.map((item, i) => (
          <li key={`${i}:${item.url}`} className="flex items-center gap-2">
            {/* eslint-disable-next-line @next/next/no-img-element -- URL storage esterno, non ottimizzabile da next/image */}
            <img src={item.url} alt="" className="h-11 w-11 shrink-0 rounded-md object-cover" />
            <input
              id={`${idBase}-alt-${i}`}
              type="text"
              aria-label="Testo alternativo"
              placeholder="Testo alternativo"
              defaultValue={item.alt}
              onBlur={(e) => setAlt(i, e.currentTarget.value.trim())}
              className="h-11 min-w-0 flex-1 rounded-md border border-border bg-background px-3 text-sm text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
            />
            <button
              type="button"
              aria-label={t("moveUp")}
              title={t("moveUp")}
              disabled={i === 0}
              onClick={() => move(i, -1)}
              className={buttonClass}
            >
              <span aria-hidden="true">↑</span>
            </button>
            <button
              type="button"
              aria-label={t("moveDown")}
              title={t("moveDown")}
              disabled={i === items.length - 1}
              onClick={() => move(i, 1)}
              className={buttonClass}
            >
              <span aria-hidden="true">↓</span>
            </button>
            <button
              type="button"
              aria-label={t("remove")}
              title={t("remove")}
              onClick={() => remove(i)}
              className={buttonClass}
            >
              <span aria-hidden="true">×</span>
            </button>
          </li>
        ))}
      </ol>

      {canAdd && (
        <button
          type="button"
          onClick={requestAdd}
          className="mt-3 flex h-11 items-center rounded-md border border-dashed border-border px-4 text-sm font-medium text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
        >
          + {tEditor("image")}
        </button>
      )}
    </NodeViewWrapper>
  );
}

export function galleryNodeView() {
  return ReactNodeViewRenderer(GalleryNodeView);
}
