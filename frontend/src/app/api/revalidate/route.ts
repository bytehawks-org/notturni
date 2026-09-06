import { timingSafeEqual } from "node:crypto";

import { revalidateTag } from "next/cache";

/** Invalidazione on-demand della cache dei Server Component pubblici.
 *
 * Chiamata dal backend (fire-and-forget) dopo una modifica a contenuti
 * pubblici — vedi `backend/app/core/revalidation.py`. Protetta da un secret
 * condiviso (`NOCT_REVALIDATE_SECRET`, identico ai due servizi): se non è
 * configurato, l'endpoint non esiste e la cache si affida solo alla finestra
 * a tempo (`REVALIDATE_SECONDS` in `src/lib/revalidate.ts`).
 *
 * Body: `{ "tags": ["blog:il-mio-blog", "feed", ...] }` — le stringhe dei tag
 * sono quelle prodotte da `revalidateTags` in `src/lib/revalidate.ts`.
 */

const MAX_TAGS = 50;
const MAX_TAG_LENGTH = 256; // limite di revalidateTag

function secretMatches(provided: string, expected: string): boolean {
  const a = Buffer.from(provided);
  const b = Buffer.from(expected);
  // timingSafeEqual esige buffer di pari lunghezza: il confronto di lunghezza
  // non è a tempo costante ma non rivela il secret, solo la sua lunghezza.
  return a.length === b.length && timingSafeEqual(a, b);
}

export async function POST(request: Request): Promise<Response> {
  const expected = process.env.NOCT_REVALIDATE_SECRET;
  if (!expected) {
    // funzionalità non configurata: non annunciarla
    return new Response("Not found", { status: 404 });
  }

  const auth = request.headers.get("authorization") ?? "";
  const provided = auth.startsWith("Bearer ") ? auth.slice(7) : "";
  if (!provided || !secretMatches(provided, expected)) {
    return new Response("Unauthorized", { status: 401 });
  }

  let body: unknown;
  try {
    body = await request.json();
  } catch {
    return Response.json({ error: "Body JSON non valido." }, { status: 400 });
  }

  const rawTags = (body as { tags?: unknown })?.tags;
  if (!Array.isArray(rawTags)) {
    return Response.json({ error: "Campo `tags` mancante o non è un array." }, { status: 400 });
  }

  const tags = Array.from(
    new Set(
      rawTags.filter(
        (t): t is string => typeof t === "string" && t.length > 0 && t.length <= MAX_TAG_LENGTH
      )
    )
  ).slice(0, MAX_TAGS);

  for (const tag of tags) {
    // "max": semantica stale-while-revalidate (il visitatore successivo vede
    // subito il contenuto ancora in cache mentre quello fresco si rigenera in
    // background) — consigliata per contenuti tipo blog.
    revalidateTag(tag, "max");
  }

  return Response.json({ revalidated: tags });
}
