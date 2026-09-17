/** Slug generico lato client (es. proposta automatica dello slug di un post
 * a partire dal titolo, editor/posts/new): minuscolo, accenti rimossi,
 * qualunque sequenza di caratteri non alfanumerici diventa un trattino.
 * Nessun vincolo di lunghezza minima/formato oltre a questo — a differenza
 * dello slug di blog (backend/app/domain/blog_rules.py), il backend non
 * impone un formato allo slug di un post, solo l'unicità per blog+lingua e
 * l'esclusione delle parole riservate (permalinks.py). */
export function slugify(text: string, maxLength = 80): string {
  return text
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, maxLength)
    .replace(/-+$/g, "");
}
