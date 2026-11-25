import { fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { renderWithIntl } from "@/test/render";

const h = vi.hoisted(() => ({
  uploadMedia: vi.fn(),
  uploadMediaMany: vi.fn(),
}));

vi.mock("@/lib/api", () => {
  class ApiClientError extends Error {}
  return {
    ApiClientError,
    api: {
      blogs: { uploadMedia: h.uploadMedia, mediaLibrary: vi.fn() },
      users: { myMedia: vi.fn() },
    },
    uploadMediaMany: h.uploadMediaMany,
  };
});

import { ImagePickerModal } from "./ImagePickerModal";

const authFetch = <T,>(fn: (token: string) => Promise<T>) => fn("token");

function makeFiles(names: string[]): File[] {
  return names.map((name) => new File(["x"], name, { type: "image/png" }));
}

function selectFiles(files: File[]) {
  const input = document.querySelector('input[type="file"]') as HTMLInputElement;
  fireEvent.change(input, { target: { files } });
}

function okOutcome(file: File) {
  return { file, ok: true, media: { url: `https://cdn.example.com/${file.name}`, is_sensitive: false } };
}

beforeEach(() => {
  h.uploadMedia.mockReset();
  h.uploadMediaMany.mockReset();
});

describe("ImagePickerModal", () => {
  it("single file: calls onSelect once with the uploaded image and closes", async () => {
    h.uploadMedia.mockResolvedValue({ url: "https://cdn.example.com/one.png", is_sensitive: true });
    const onSelect = vi.fn();
    const onPickMany = vi.fn();
    const onClose = vi.fn();
    renderWithIntl(
      <ImagePickerModal blogSlug="blog" authFetch={authFetch} onSelect={onSelect} onPickMany={onPickMany} onClose={onClose} />,
    );

    selectFiles(makeFiles(["one.png"]));

    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
    expect(onSelect).toHaveBeenCalledWith({ url: "https://cdn.example.com/one.png", is_sensitive: true });
    expect(onPickMany).not.toHaveBeenCalled();
    expect(h.uploadMediaMany).not.toHaveBeenCalled();
  });

  it("three files: calls onPickMany once with the three images in order and closes", async () => {
    const files = makeFiles(["a.png", "b.png", "c.png"]);
    h.uploadMediaMany.mockImplementation(async (_token: string, _slug: string, list: File[]) => list.map(okOutcome));
    const onSelect = vi.fn();
    const onPickMany = vi.fn();
    const onClose = vi.fn();
    renderWithIntl(
      <ImagePickerModal blogSlug="blog" authFetch={authFetch} onSelect={onSelect} onPickMany={onPickMany} onClose={onClose} />,
    );

    selectFiles(files);

    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
    expect(onPickMany).toHaveBeenCalledTimes(1);
    expect(onPickMany).toHaveBeenCalledWith([
      { url: "https://cdn.example.com/a.png", is_sensitive: false },
      { url: "https://cdn.example.com/b.png", is_sensitive: false },
      { url: "https://cdn.example.com/c.png", is_sensitive: false },
    ]);
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("eleven files: shows the too-many message and uploads nothing", () => {
    const onPickMany = vi.fn();
    const onClose = vi.fn();
    renderWithIntl(
      <ImagePickerModal blogSlug="blog" authFetch={authFetch} onSelect={vi.fn()} onPickMany={onPickMany} onClose={onClose} />,
    );

    selectFiles(makeFiles(Array.from({ length: 11 }, (_, i) => `f${i}.png`)));

    expect(screen.getByText(/at most 10 images/)).toBeInTheDocument();
    expect(h.uploadMediaMany).not.toHaveBeenCalled();
    expect(onPickMany).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("one failing file: shows its error, returns the others, stays open and retries it", async () => {
    const files = makeFiles(["a.png", "b.png", "c.png"]);
    h.uploadMediaMany.mockImplementation(async (_token: string, _slug: string, list: File[]) =>
      list.map((file) =>
        file.name === "b.png"
          ? { file, ok: false, error: "Quota exceeded" }
          : okOutcome(file),
      ),
    );
    const onPickMany = vi.fn();
    const onClose = vi.fn();
    renderWithIntl(
      <ImagePickerModal blogSlug="blog" authFetch={authFetch} onSelect={vi.fn()} onPickMany={onPickMany} onClose={onClose} />,
    );

    selectFiles(files);

    expect(await screen.findByText("Quota exceeded")).toBeInTheDocument();
    expect(onPickMany).toHaveBeenCalledWith([
      { url: "https://cdn.example.com/a.png", is_sensitive: false },
      { url: "https://cdn.example.com/c.png", is_sensitive: false },
    ]);
    expect(onClose).not.toHaveBeenCalled();

    h.uploadMediaMany.mockImplementation(async (_token: string, _slug: string, list: File[]) => list.map(okOutcome));
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));

    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
    expect(onPickMany).toHaveBeenLastCalledWith([{ url: "https://cdn.example.com/b.png", is_sensitive: false }]);
  });
});
