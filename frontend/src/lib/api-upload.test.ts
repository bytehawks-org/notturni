import { afterEach, describe, expect, it, vi } from "vitest";

import { ApiClientError, api, uploadMediaMany } from "@/lib/api";
import { GALLERY_MAX } from "@/lib/gallery";

// Synthetic files only: no real image data, no network (uploadMedia is mocked).
function makeFiles(n: number): File[] {
  return Array.from({ length: n }, (_, i) => new File([`img-${i}`], `img-${i}.jpg`, { type: "image/jpeg" }));
}

type Deferred<T> = { promise: Promise<T>; resolve: (v: T) => void; reject: (e: unknown) => void };
function deferred<T>(): Deferred<T> {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const okMedia = (url: string) => ({ url, is_sensitive: false });

afterEach(() => {
  vi.restoreAllMocks();
});

describe("uploadMediaMany", () => {
  it("returns an empty list without any request when given no files", async () => {
    const spy = vi.spyOn(api.blogs, "uploadMedia");
    await expect(uploadMediaMany("tok", "blog", [])).resolves.toEqual([]);
    expect(spy).not.toHaveBeenCalled();
  });

  it("refuses more than GALLERY_MAX files up front with zero requests", async () => {
    const spy = vi.spyOn(api.blogs, "uploadMedia");
    const files = makeFiles(GALLERY_MAX + 1);
    expect(files).toHaveLength(11);
    await expect(uploadMediaMany("tok", "blog", files)).rejects.toBeInstanceOf(RangeError);
    expect(spy).not.toHaveBeenCalled();
  });

  it("accepts exactly GALLERY_MAX files", async () => {
    vi.spyOn(api.blogs, "uploadMedia").mockImplementation(async (_t, _s, file) => okMedia(file.name));
    const results = await uploadMediaMany("tok", "blog", makeFiles(GALLERY_MAX));
    expect(results).toHaveLength(GALLERY_MAX);
    expect(results.every((r) => r.ok)).toBe(true);
  });

  it("rejects a non-positive or non-integer concurrency with RangeError and zero requests", async () => {
    const spy = vi.spyOn(api.blogs, "uploadMedia");
    await expect(uploadMediaMany("tok", "blog", makeFiles(2), { concurrency: 0 })).rejects.toBeInstanceOf(RangeError);
    await expect(uploadMediaMany("tok", "blog", makeFiles(2), { concurrency: 1.5 })).rejects.toBeInstanceOf(RangeError);
    expect(spy).not.toHaveBeenCalled();
  });

  it("calls uploadMedia with token, slug and each file", async () => {
    const spy = vi.spyOn(api.blogs, "uploadMedia").mockImplementation(async (_t, _s, file) => okMedia(file.name));
    const files = makeFiles(2);
    const results = await uploadMediaMany("tok", "my-blog", files);
    expect(spy).toHaveBeenNthCalledWith(1, "tok", "my-blog", files[0]);
    expect(spy).toHaveBeenNthCalledWith(2, "tok", "my-blog", files[1]);
    expect(results[0]).toEqual({ file: files[0], ok: true, media: okMedia("img-0.jpg") });
  });

  it("preserves input order when uploads complete out of order", async () => {
    const files = makeFiles(3);
    const pending = files.map(() => deferred<{ url: string; is_sensitive: boolean }>());
    const spy = vi.spyOn(api.blogs, "uploadMedia").mockImplementation((_t, _s, file) => {
      const idx = files.indexOf(file);
      return pending[idx].promise;
    });

    const run = uploadMediaMany("tok", "blog", files, { concurrency: 3 });
    await vi.waitFor(() => expect(spy).toHaveBeenCalledTimes(3));

    // Complete in reverse order: 2, 0, 1.
    pending[2].resolve(okMedia("u2"));
    pending[0].resolve(okMedia("u0"));
    pending[1].resolve(okMedia("u1"));

    const results = await run;
    expect(results.map((r) => (r.ok ? r.media.url : null))).toEqual(["u0", "u1", "u2"]);
    expect(results.map((r) => r.file)).toEqual(files);
  });

  it("marks one rejected file as ok:false and lets the others succeed", async () => {
    const files = makeFiles(3);
    vi.spyOn(api.blogs, "uploadMedia").mockImplementation(async (_t, _s, file) => {
      if (file === files[1]) throw new ApiClientError(413, "File troppo grande");
      return okMedia(file.name);
    });

    const results = await uploadMediaMany("tok", "blog", files);
    expect(results[0]).toMatchObject({ ok: true });
    expect(results[1]).toEqual({ file: files[1], ok: false, error: "File troppo grande" });
    expect(results[2]).toMatchObject({ ok: true });
  });

  it("reports progress exactly total times, with a growing done count", async () => {
    vi.spyOn(api.blogs, "uploadMedia").mockImplementation(async (_t, _s, file) => okMedia(file.name));
    const onProgress = vi.fn();
    await uploadMediaMany("tok", "blog", makeFiles(4), { onProgress });
    expect(onProgress).toHaveBeenCalledTimes(4);
    expect(onProgress.mock.calls).toEqual([
      [1, 4],
      [2, 4],
      [3, 4],
      [4, 4],
    ]);
  });

  it("never exceeds the concurrency limit and fills it by default (3)", async () => {
    let inFlight = 0;
    let maxInFlight = 0;
    vi.spyOn(api.blogs, "uploadMedia").mockImplementation(async (_t, _s, file) => {
      inFlight += 1;
      maxInFlight = Math.max(maxInFlight, inFlight);
      await new Promise((r) => setTimeout(r, 5));
      inFlight -= 1;
      return okMedia(file.name);
    });

    const results = await uploadMediaMany("tok", "blog", makeFiles(8));
    expect(results.every((r) => r.ok)).toBe(true);
    expect(maxInFlight).toBe(3);
  });

  it("honours an explicit concurrency limit", async () => {
    let inFlight = 0;
    let maxInFlight = 0;
    vi.spyOn(api.blogs, "uploadMedia").mockImplementation(async (_t, _s, file) => {
      inFlight += 1;
      maxInFlight = Math.max(maxInFlight, inFlight);
      await new Promise((r) => setTimeout(r, 5));
      inFlight -= 1;
      return okMedia(file.name);
    });

    await uploadMediaMany("tok", "blog", makeFiles(6), { concurrency: 2 });
    expect(maxInFlight).toBe(2);
  });

  it("stops files that have not started yet when the signal aborts", async () => {
    const files = makeFiles(3);
    const controller = new AbortController();
    const spy = vi.spyOn(api.blogs, "uploadMedia").mockImplementation(async (_t, _s, file) => {
      controller.abort();
      return okMedia(file.name);
    });

    const results = await uploadMediaMany("tok", "blog", files, { concurrency: 1, signal: controller.signal });
    expect(spy).toHaveBeenCalledTimes(1);
    expect(results[0]).toMatchObject({ ok: true });
    expect(results[1]).toMatchObject({ ok: false, file: files[1] });
    expect(results[2]).toMatchObject({ ok: false, file: files[2] });
  });

  it("reports progress for files skipped by an abort, reaching total", async () => {
    const controller = new AbortController();
    controller.abort();
    vi.spyOn(api.blogs, "uploadMedia");
    const onProgress = vi.fn();
    const results = await uploadMediaMany("tok", "blog", makeFiles(2), { signal: controller.signal, onProgress });
    expect(results.every((r) => !r.ok)).toBe(true);
    expect(onProgress).toHaveBeenLastCalledWith(2, 2);
    expect(api.blogs.uploadMedia).not.toHaveBeenCalled();
  });
});
