/** Categorie di avviso sui contenuti, stile Bluesky (CLAUDE.md #3) — stesso
 * vocabolario di backend/app/domain/content_media.py::SENSITIVITY_CATEGORIES.
 * Viaggiano nel `title` Markdown delle immagini incorporate nel contenuto
 * (`![alt](url "sensitive:nudity,explicit")`, vedi frontend/src/lib/markdown.ts
 * per la resa sfocata/cliccabile sulla pagina pubblica), o in un campo
 * dedicato (`cover_image_categories`) per l'immagine di copertina, che non
 * fa parte del contenuto Markdown. */
export const SENSITIVITY_CATEGORIES = ["suggestive", "nudity", "explicit", "other"] as const;

export type SensitivityCategory = (typeof SENSITIVITY_CATEGORIES)[number];

export const SENSITIVITY_CATEGORY_GROUPS: { heading: string; categories: SensitivityCategory[] }[] = [
  { heading: "Contenuto per adulti", categories: ["suggestive", "nudity", "explicit"] },
  { heading: "Altro", categories: ["other"] },
];

export const SENSITIVITY_CATEGORY_LABELS: Record<SensitivityCategory, string> = {
  suggestive: "Suggestivo",
  nudity: "Nudità",
  explicit: "Esplicito",
  other: "Contenuto sensibile",
};

/** Estrae le categorie da un `title` Markdown (`"sensitive"` o
 * `"sensitive:cat1,cat2"`) — stesso parsing di
 * backend/app/domain/content_media.py::parse_sensitivity_categories. */
export function parseSensitivityCategories(title: string | null | undefined): SensitivityCategory[] {
  if (!title || !title.startsWith("sensitive")) return [];
  const raw = title.slice("sensitive".length).replace(/^:/, "");
  if (!raw) return [];
  return raw
    .split(",")
    .map((c) => c.trim())
    .filter((c): c is SensitivityCategory => (SENSITIVITY_CATEGORIES as readonly string[]).includes(c));
}

export function isFlaggedSensitive(title: string | null | undefined): boolean {
  return !!title && title.startsWith("sensitive");
}

/** Costruisce il `title` Markdown da riattaccare all'immagine a partire
 * dalle categorie scelte nel modal — `"sensitive"` da solo se nessuna
 * categoria specifica è stata indicata (equivalente alla sola segnalazione
 * automatica). `undefined` se la lista è vuota E non si vuole più
 * segnalare nulla: gestito dal chiamante, non da questa funzione. */
export function buildSensitiveTitle(categories: SensitivityCategory[]): string {
  return categories.length > 0 ? `sensitive:${categories.join(",")}` : "sensitive";
}
