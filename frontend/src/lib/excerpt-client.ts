/** Estratto in solo testo lato client (nessun DOMPurify/JSDOM: lib/markdown.ts
 * è `server-only`). Approssimazione con regex, sufficiente per le anteprime
 * dei feed renderizzati nel browser (feed "Seguiti", tab del profilo). */
export function excerptClient(markdown: string, maxLength = 160): string {
  const text = markdown
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, " ")
    .replace(/\[(\d{1,3})\]\(#nota-\d{1,3}\)/g, "")
    .replace(/\[\^\d{1,3}\]/g, "")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/[#>*_`~|]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
  return text.length <= maxLength ? text : `${text.slice(0, maxLength).trimEnd()}…`;
}
