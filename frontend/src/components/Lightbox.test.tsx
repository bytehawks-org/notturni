import { act, fireEvent, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { LightboxProvider, useLightbox } from "@/components/Lightbox";
import { renderWithIntl } from "@/test/render";

function tile(gid: string, i: number, sensitive = false, revealed = false) {
  const img = `<img src="/${gid}-${i}.jpg" alt="alt ${gid}-${i}" data-gallery="${gid}" data-gallery-index="${i}" ${
    sensitive ? "" : 'data-lightbox="1" tabindex="0" role="button"'
  }>`;
  if (!sensitive) return `<div class="post-gallery__tile">${img}</div>`;
  return `<label class="sensitive-image-wrapper"><input type="checkbox" class="sensitive-image-toggle"${
    revealed ? " checked" : ""
  }>${img}<button type="button" class="lightbox-expand-btn" data-lightbox-src="/${gid}-${i}.jpg" data-lightbox-alt="alt ${gid}-${i}" data-gallery="${gid}" data-gallery-index="${i}">x</button></label>`;
}

function gallery(gid: string, tiles: string[]) {
  return `<div class="post-gallery" data-gallery="${gid}" role="group">${tiles.join("")}</div>`;
}

function Content({ html }: { html: string }) {
  return <div data-testid="content" dangerouslySetInnerHTML={{ __html: html }} />;
}

function Cover() {
  const open = useLightbox();
  return (
    <button type="button" onClick={() => open("/cover.jpg", "cover")}>
      cover
    </button>
  );
}

function setup(html: string) {
  return renderWithIntl(
    <LightboxProvider>
      <Content html={html} />
      <Cover />
    </LightboxProvider>,
  );
}

const dialogImg = () => screen.getByRole("dialog").querySelector("img")!;

describe("Lightbox gallery", () => {
  const html = gallery("g1", [tile("g1", 0), tile("g1", 1), tile("g1", 2)]) + gallery("g1", [tile("g1", 0), tile("g1", 1)]);

  it("shows the counter and navigates with arrow keys, stopping at the ends", () => {
    setup(html);
    const first = document.querySelectorAll("img")[1];
    fireEvent.click(first);
    expect(screen.getByText("2 / 3")).toBeTruthy();
    expect(dialogImg().getAttribute("src")).toBe("/g1-1.jpg");
    fireEvent.keyDown(document, { key: "ArrowRight" });
    expect(screen.getByText("3 / 3")).toBeTruthy();
    fireEvent.keyDown(document, { key: "ArrowRight" });
    expect(screen.getByText("3 / 3")).toBeTruthy();
    expect((screen.getByLabelText("Next image") as HTMLButtonElement).disabled).toBe(true);
    fireEvent.keyDown(document, { key: "Home" });
    expect(screen.getByText("1 / 3")).toBeTruthy();
    fireEvent.keyDown(document, { key: "ArrowLeft" });
    expect(screen.getByText("1 / 3")).toBeTruthy();
    fireEvent.keyDown(document, { key: "End" });
    expect(screen.getByText("3 / 3")).toBeTruthy();
  });

  it("scopes the group to the closest .post-gallery when ids collide", () => {
    setup(html);
    fireEvent.click(document.querySelectorAll("img")[3]);
    expect(screen.getByText("1 / 2")).toBeTruthy();
  });

  it("navigates with the buttons", () => {
    setup(html);
    fireEvent.click(document.querySelectorAll("img")[0]);
    fireEvent.click(screen.getByLabelText("Next image"));
    expect(dialogImg().getAttribute("src")).toBe("/g1-1.jpg");
    fireEvent.click(screen.getByLabelText("Previous image"));
    expect(dialogImg().getAttribute("src")).toBe("/g1-0.jpg");
  });

  it("swipes", () => {
    setup(html);
    fireEvent.click(document.querySelectorAll("img")[0]);
    const d = screen.getByRole("dialog");
    fireEvent.touchStart(d, { touches: [{ clientX: 200 }] });
    fireEvent.touchEnd(d, { changedTouches: [{ clientX: 100 }] });
    expect(screen.getByText("2 / 3")).toBeTruthy();
    fireEvent.touchStart(d, { touches: [{ clientX: 100 }] });
    fireEvent.touchEnd(d, { changedTouches: [{ clientX: 120 }] });
    expect(screen.getByText("2 / 3")).toBeTruthy();
  });

  it("focuses close on open, traps Tab, restores focus on Escape", () => {
    setup(html);
    const opener = document.querySelectorAll("img")[0] as HTMLElement;
    opener.focus();
    fireEvent.click(opener);
    const dialog = screen.getByRole("dialog");
    expect(dialog.getAttribute("aria-modal")).toBe("true");
    expect(document.activeElement).toBe(screen.getByLabelText("Close"));
    fireEvent.keyDown(document, { key: "Tab", shiftKey: true });
    expect(document.activeElement).toBe(screen.getByLabelText("Next image"));
    fireEvent.keyDown(document, { key: "Tab" });
    expect(document.activeElement).toBe(screen.getByLabelText("Close"));
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(document.activeElement).toBe(opener);
  });

  it("skips unrevealed sensitive tiles and includes revealed ones", () => {
    setup(gallery("g1", [tile("g1", 0), tile("g1", 1, true, false), tile("g1", 2)]));
    fireEvent.click(document.querySelectorAll("img")[0]);
    expect(screen.getByText("1 / 2")).toBeTruthy();
    fireEvent.keyDown(document, { key: "ArrowRight" });
    expect(dialogImg().getAttribute("src")).toBe("/g1-2.jpg");
  });

  it("opens a revealed sensitive tile from its expand button", () => {
    setup(gallery("g1", [tile("g1", 0), tile("g1", 1, true, true), tile("g1", 2)]));
    fireEvent.click(document.querySelector(".lightbox-expand-btn")!);
    expect(screen.getByText("2 / 3")).toBeTruthy();
  });
});

describe("Lightbox single images (no regression)", () => {
  it("opens a standalone image without navigation and closes on Escape", () => {
    setup('<img src="/a.jpg" alt="a" data-lightbox="1" tabindex="0">');
    fireEvent.click(document.querySelector("img")!);
    expect(dialogImg().getAttribute("src")).toBe("/a.jpg");
    expect(screen.queryByLabelText("Next image")).toBeNull();
    expect(screen.queryByLabelText("Previous image")).toBeNull();
    expect(screen.queryByText(/ \/ /)).toBeNull();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("opens with Enter on a standalone image and closes on backdrop click", () => {
    setup('<img src="/a.jpg" alt="a" data-lightbox="1" tabindex="0">');
    fireEvent.keyDown(document.querySelector("img")!, { key: "Enter" });
    expect(screen.getByRole("dialog")).toBeTruthy();
    fireEvent.click(screen.getByRole("dialog"));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("opens a standalone sensitive image via its expand button", () => {
    setup(
      '<label class="sensitive-image-wrapper"><input type="checkbox" class="sensitive-image-toggle" checked><img src="/s.jpg" alt="s"><button type="button" class="lightbox-expand-btn" data-lightbox-src="/s.jpg" data-lightbox-alt="s">x</button></label>',
    );
    fireEvent.click(document.querySelector(".lightbox-expand-btn")!);
    expect(dialogImg().getAttribute("src")).toBe("/s.jpg");
    expect(screen.queryByLabelText("Next image")).toBeNull();
  });

  it("opens via useLightbox (cover) and restores focus to the opener", () => {
    setup("");
    const btn = screen.getByText("cover");
    btn.focus();
    act(() => btn.click());
    expect(dialogImg().getAttribute("src")).toBe("/cover.jpg");
    fireEvent.click(screen.getByLabelText("Close"));
    expect(document.activeElement).toBe(btn);
  });
});
