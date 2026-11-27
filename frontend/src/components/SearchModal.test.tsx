import { act, fireEvent, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import { SearchModal } from "@/components/SearchModal";
import { SEARCH_DEBOUNCE_MS, type SearchScope } from "@/lib/search";
import en from "@/messages/en.json";
import it_ from "@/messages/it.json";
import { renderWithIntl } from "@/test/render";

const push = vi.hoisted(() => vi.fn());
vi.mock("next/navigation", () => ({ useRouter: () => ({ push }) }));

const BLOG: SearchScope = { kind: "blog", slug: "foo", basePath: "/foo", title: "Foo Blog" };
const SOLO: SearchScope = { kind: "blog", slug: "foo", basePath: "" };

// jsdom has no `showModal()`/`close()`: minimal stubs that toggle `open`.
const proto = HTMLDialogElement.prototype;
const original = { showModal: proto.showModal, close: proto.close };
const showModal = vi.fn(function (this: HTMLDialogElement) {
  this.setAttribute("open", "");
});

beforeAll(() => {
  proto.close = function (this: HTMLDialogElement) {
    this.removeAttribute("open");
  };
});
afterAll(() => {
  Object.assign(proto, original);
});

function post(id: string) {
  return { id, title: `Title ${id}`, blog_slug: "foo", permalink: `/foo/${id}`, published_at: "2026-03-05T10:00:00Z" };
}

function stubFetch(status: number, body: unknown) {
  const fetchMock = vi.fn(async () =>
    new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } }),
  );
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

function Harness({ scope }: { scope?: SearchScope }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        trigger
      </button>
      <SearchModal open={open} onClose={() => setOpen(false)} scope={scope} />
    </>
  );
}

async function openModal(scope?: SearchScope) {
  const user = userEvent.setup();
  renderWithIntl(<Harness scope={scope} />);
  await user.click(screen.getByRole("button", { name: "trigger" }));
  return { user, input: screen.getByRole("combobox") };
}

/** Types a query and waits for the debounce and the answer. */
async function search(q: string) {
  fireEvent.change(screen.getByRole("combobox"), { target: { value: q } });
  await act(() => new Promise((resolve) => setTimeout(resolve, SEARCH_DEBOUNCE_MS + 50)));
}

beforeEach(() => {
  proto.showModal = showModal;
  push.mockClear();
  showModal.mockClear();
});
afterEach(() => {
  vi.unstubAllGlobals();
});

describe("SearchModal", () => {
  it("renders nothing when closed", () => {
    renderWithIntl(<SearchModal open={false} onClose={() => {}} />);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("opens as a modal dialog, focuses the input and locks body scroll", async () => {
    const { input } = await openModal();
    expect(showModal).toHaveBeenCalled();
    expect(screen.getByRole("dialog", { name: "Search" })).toBeInTheDocument();
    expect(input).toHaveFocus();
    expect(input).toHaveAttribute("placeholder", en.SearchPage.placeholder);
    expect(document.body.style.overflow).toBe("hidden");
  });

  it("uses the blog title and placeholder for a blog scope", async () => {
    const { input } = await openModal(BLOG);
    expect(screen.getByRole("dialog", { name: "Search Foo Blog" })).toBeInTheDocument();
    expect(input).toHaveAttribute("placeholder", en.BlogSearchPage.placeholder);
  });

  it("closes on Escape, restores focus and body scroll", async () => {
    const { user } = await openModal();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "trigger" })).toHaveFocus();
    expect(document.body.style.overflow).toBe("");
  });

  it("closes on the dialog cancel event", async () => {
    await openModal();
    fireEvent(screen.getByRole("dialog"), new Event("cancel", { cancelable: true }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("closes on a backdrop click but not on a content click", async () => {
    const { user } = await openModal();
    await user.click(screen.getByRole("heading"));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    await user.click(screen.getByRole("dialog"));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "trigger" })).toHaveFocus();
  });

  it("renders results with combobox and listbox semantics", async () => {
    stubFetch(200, [post("a"), post("b")]);
    const { input } = await openModal();
    expect(input).toHaveAttribute("aria-expanded", "false");
    expect(input).toHaveAttribute("aria-autocomplete", "list");
    await search("ti");

    expect(input).toHaveAttribute("aria-expanded", "true");
    expect(input).toHaveAttribute("aria-controls", screen.getByRole("listbox").id);
    expect(input).not.toHaveAttribute("aria-activedescendant");
    const options = screen.getAllByRole("option");
    expect(options).toHaveLength(2);
    expect(options[0]).toHaveAttribute("aria-selected", "false");
    expect(options[0]).toHaveAttribute("href", "/foo/a");
    expect(options[0]).toHaveTextContent("Title a");
    expect(options[0]).toHaveTextContent("foo");
    expect(options[0]).toHaveTextContent("Mar 5, 2026");
    expect(screen.getByRole("status")).toHaveTextContent("2 suggestions");
  });

  it("omits the blog slug in a blog scope and uses the singular count", async () => {
    stubFetch(200, [post("a")]);
    await openModal(SOLO);
    await search("ti");
    expect(screen.getByRole("option").textContent).toBe("Title aMar 5, 2026");
    expect(screen.getByRole("status")).toHaveTextContent(/^1 suggestion$/);
  });

  it("renders titles as plain text", async () => {
    stubFetch(200, [{ ...post("a"), title: "<img src=x>" }]);
    await openModal();
    await search("ti");
    expect(screen.getByRole("option")).toHaveTextContent("<img src=x>");
    expect(screen.getByRole("dialog").querySelector("img")).toBeNull();
  });

  it("moves with the arrows, wraps, and opens the active post with Enter", async () => {
    stubFetch(200, [post("a"), post("b")]);
    const { user, input } = await openModal();
    await search("ti");
    const [first, second] = screen.getAllByRole("option");

    await user.keyboard("{ArrowDown}");
    expect(input).toHaveAttribute("aria-activedescendant", first.id);
    expect(first).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{ArrowDown}{ArrowDown}");
    expect(input).toHaveAttribute("aria-activedescendant", first.id);
    await user.keyboard("{ArrowUp}");
    expect(input).toHaveAttribute("aria-activedescendant", second.id);
    expect(first).toHaveAttribute("aria-selected", "false");
    expect(input).toHaveFocus();

    await user.keyboard("{Enter}");
    expect(push).toHaveBeenCalledExactlyOnceWith("/foo/b");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("selects the last option with Up from no selection", async () => {
    stubFetch(200, [post("a"), post("b")]);
    const { user, input } = await openModal();
    await search("ti");
    await user.keyboard("{ArrowUp}");
    expect(input).toHaveAttribute("aria-activedescendant", screen.getAllByRole("option")[1].id);
  });

  it("resets the selection when the query and results change", async () => {
    stubFetch(200, [post("a")]);
    const { user, input } = await openModal();
    await search("ti");
    await user.keyboard("{ArrowDown}");
    await search("tit");
    expect(screen.getByRole("option")).toHaveAttribute("aria-selected", "false");
    expect(input).not.toHaveAttribute("aria-activedescendant");
  });

  it("opens a post on click", async () => {
    stubFetch(200, [post("a")]);
    const { user } = await openModal();
    await search("ti");
    await user.click(screen.getByRole("option"));
    expect(push).toHaveBeenCalledExactlyOnceWith("/foo/a");
  });

  it("ignores Enter during IME composition", async () => {
    stubFetch(200, [post("a")]);
    const { user, input } = await openModal();
    await search("ti");
    await user.keyboard("{ArrowDown}");
    expect(fireEvent.keyDown(input, { key: "Enter", isComposing: true })).toBe(false);
    expect(fireEvent.keyDown(input, { key: "Enter", keyCode: 229 })).toBe(false);
    expect(push).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it.each([
    ["portal", undefined, "/search?q=a%20%26b"],
    ["blog", BLOG, "/foo/search?q=a%20%26b"],
    ["solo primary blog", SOLO, "/search?q=a%20%26b"],
  ])("submits to the %s results page with Enter and with the button", async (_name, scope, href) => {
    stubFetch(200, []);
    const { user } = await openModal(scope);
    await user.type(screen.getByRole("combobox"), "  a &b {Enter}");
    expect(push).toHaveBeenLastCalledWith(href);

    await user.click(screen.getByRole("button", { name: "trigger" }));
    expect(screen.getByRole("combobox")).toHaveValue("");
    await user.type(screen.getByRole("combobox"), "a &b");
    await user.click(screen.getByRole("button", { name: "Search" }));
    expect(push).toHaveBeenCalledTimes(2);
    expect(push).toHaveBeenLastCalledWith(href);
  });

  it("disables the submit button for an empty query", async () => {
    const { user } = await openModal();
    expect(screen.getByRole("button", { name: "Search" })).toBeDisabled();
    await user.type(screen.getByRole("combobox"), "   {Enter}");
    expect(screen.getByRole("button", { name: "Search" })).toBeDisabled();
    expect(push).not.toHaveBeenCalled();
  });

  it("announces type more and loading", async () => {
    stubFetch(200, []);
    await openModal();
    expect(screen.getByRole("status")).toBeEmptyDOMElement();
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "t" } });
    expect(screen.getByRole("status")).toHaveTextContent("Type at least 2 characters.");
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "ti" } });
    expect(screen.getByRole("status")).toHaveTextContent(en.Common.loading);
  });

  it.each([
    [200, [], "No results for \"ti\"."],
    [500, { detail: "boom" }, en.SearchModal.error],
    [429, { detail: "slow" }, en.SearchModal.rateLimited],
  ])("announces the outcome of an HTTP %i answer without options", async (status, body, message) => {
    stubFetch(status, body);
    const { input } = await openModal();
    await search("ti");
    expect(screen.getByRole("status")).toHaveTextContent(message);
    expect(screen.queryByRole("option")).not.toBeInTheDocument();
    expect(input).toHaveAttribute("aria-expanded", "false");
  });
});

describe("messages", () => {
  function keys(value: object, prefix = ""): string[] {
    return Object.entries(value)
      .flatMap(([k, v]) => (typeof v === "object" && v !== null ? keys(v, `${prefix}${k}.`) : [`${prefix}${k}`]))
      .sort();
  }

  it("has the same keys in both languages", () => {
    expect(keys(it_)).toEqual(keys(en));
  });
});
