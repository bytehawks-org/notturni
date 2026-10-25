/** Testi dell'interfaccia riscrivibili dagli amministratori (`/admin/testi`). I valori predefiniti restano nei
 * file di traduzione; un override sostituisce solo la stringa con quella chiave (percorso con i punti, es.
 * `Home.pillars.noAi.title`). Le regole sono le stesse del backend (`noct_platform::rules`): testo semplice,
 * senza graffe né angolari, perché i messaggi passano da un renderer ICU che altrimenti si spezzerebbe. */

export interface TextGroup {
  id: "home" | "newsletter";
  namespace: string;
}

/** Gruppi ammessi: devono coincidere con `EDITABLE_PREFIXES` del backend. */
export const TEXT_GROUPS: TextGroup[] = [
  { id: "home", namespace: "Home" },
  { id: "newsletter", namespace: "NewsletterSignup" },
];

export const MAX_TEXT_LENGTH = 2000;

/** Tag ammessi per chiave (come `RICH_TAGS` del backend): il componente fornisce il renderer del tag. */
export const RICH_TAGS: Record<string, string[]> = { "NewsletterSignup.consent": ["privacy"] };

type Messages = { [key: string]: Messages | string };

/** `true` se `value` è un testo accettabile per `key`: una riga, senza graffe né angolari, salvo i tag di
 * `RICH_TAGS` ben formati (aperti e chiusi una sola volta, in ordine). */
export function isPlainText(value: string, key = ""): boolean {
  if (value.length > MAX_TEXT_LENGTH || value.includes("'<") || value.includes("'>")) return false;
  let rest = value;
  for (const tag of RICH_TAGS[key] ?? []) {
    const open = `<${tag}>`;
    const close = `</${tag}>`;
    const o = rest.split(open).length - 1;
    const c = rest.split(close).length - 1;
    const ordered = o === 0 || c === 0 || rest.indexOf(open) < rest.indexOf(close);
    if (o !== c || o > 1 || !ordered) return false;
    rest = rest.split(open).join("").split(close).join("");
  }
  return !/[{}<>\u0000-\u001f\u007f]/.test(rest);
}

/** Le stringhe modificabili di un gruppo: foglie testuali senza segnaposto (un testo con `{count}` non si può
 * riscrivere senza rompere il messaggio), con il loro percorso completo. */
export function editableEntries(messages: Messages, namespace: string): { key: string; defaultText: string }[] {
  const out: { key: string; defaultText: string }[] = [];
  const walk = (node: Messages | string, path: string) => {
    if (typeof node === "string") {
      if (isPlainText(node, path)) out.push({ key: path, defaultText: node });
      return;
    }
    for (const [k, v] of Object.entries(node)) walk(v, `${path}.${k}`);
  };
  const root = messages[namespace];
  if (root && typeof root === "object") walk(root, namespace);
  return out;
}

/** Copia di `messages` con gli override applicati. Si ignorano chiavi inesistenti, non testuali o non ammesse:
 * un override malformato non deve mai rompere una pagina. */
export function applySiteTexts<T extends object>(messages: T, overrides: Record<string, unknown>): T {
  const prefixes = TEXT_GROUPS.map((g) => `${g.namespace}.`);
  const out = structuredClone(messages) as Messages;
  for (const [key, value] of Object.entries(overrides)) {
    if (typeof value !== "string" || !isPlainText(value, key) || !prefixes.some((p) => key.startsWith(p))) continue;
    const parts = key.split(".");
    const last = parts.pop() as string;
    let node: Messages | string | undefined = out;
    for (const part of parts) node = typeof node === "object" ? node[part] : undefined;
    if (typeof node === "object" && typeof node[last] === "string") node[last] = value;
  }
  return out as T;
}
