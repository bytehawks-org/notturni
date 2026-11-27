import { fireEvent, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { SearchTrigger } from "@/components/SearchTrigger";
import { renderWithIntl } from "@/test/render";

// The modal itself is covered by SearchModal.test.tsx: here only its props matter.
const modalProps = vi.hoisted(() => vi.fn());
// It keeps its own state, like the real modal: a modal that stays mounted keeps the typed query.
vi.mock("@/components/SearchModal", async () => {
  const { useState } = await import("react");
  return {
    SearchModal: function MockSearchModal(props: { open: boolean; onClose: () => void; scope?: unknown }) {
      const [query, setQuery] = useState("");
      modalProps(props);
      if (!props.open) return null;
      return (
        <div role="dialog" aria-label="modal">
          <input aria-label="query" value={query} onChange={(e) => setQuery(e.target.value)} />
          <button type="button" onClick={props.onClose}>
            close
          </button>
        </div>
      );
    },
  };
});

const label = "Search";

describe("SearchTrigger", () => {
  it("is a link to the blog search page with a dialog hint", () => {
    renderWithIntl(<SearchTrigger blogSlug="foo" basePath="/foo" title="Foo">{label}</SearchTrigger>);
    const link = screen.getByRole("link", { name: label });
    expect(link).toHaveAttribute("href", "/foo/search");
    expect(link).toHaveAttribute("aria-haspopup", "dialog");
  });

  it("uses /search on the solo primary blog (empty basePath)", () => {
    renderWithIntl(<SearchTrigger blogSlug="notturni" basePath="">{label}</SearchTrigger>);
    expect(screen.getByRole("link", { name: label })).toHaveAttribute("href", "/search");
  });

  it("plain click opens the modal with the blog scope and prevents navigation", async () => {
    const user = userEvent.setup();
    renderWithIntl(<SearchTrigger blogSlug="foo" basePath="/foo" title="Foo">{label}</SearchTrigger>);
    const link = screen.getByRole("link", { name: label });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    await user.click(link);

    expect(screen.getByRole("dialog", { name: "modal" })).toBeInTheDocument();
    expect(modalProps).toHaveBeenLastCalledWith(
      expect.objectContaining({
        open: true,
        scope: { kind: "blog", slug: "foo", basePath: "/foo", title: "Foo" },
      }),
    );
  });

  it("starts from an empty modal when it is opened again", async () => {
    const user = userEvent.setup();
    renderWithIntl(<SearchTrigger blogSlug="foo" basePath="/foo">{label}</SearchTrigger>);
    const link = screen.getByRole("link", { name: label });
    await user.click(link);
    await user.type(screen.getByRole("textbox", { name: "query" }), "lanterna");
    await user.click(screen.getByRole("button", { name: "close" }));

    await user.click(link);

    expect(screen.getByRole("textbox", { name: "query" })).toHaveValue("");
  });

  it.each([
    ["ctrl", { ctrlKey: true }],
    ["meta", { metaKey: true }],
    ["shift", { shiftKey: true }],
    ["alt", { altKey: true }],
    ["middle", { button: 1 }],
  ])("%s click keeps the link behaviour and does not open the modal", (_name, init) => {
    renderWithIntl(<SearchTrigger blogSlug="foo" basePath="/foo">{label}</SearchTrigger>);
    const link = screen.getByRole("link", { name: label });

    const notPrevented = fireEvent.click(link, init);

    expect(notPrevented).toBe(true);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
