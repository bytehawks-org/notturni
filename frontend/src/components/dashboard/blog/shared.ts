import { ApiClientError } from "@/lib/api";
import type { BlogRole } from "@/lib/types";

/** Messaggio d'errore leggibile dai vari tab della scheda blog del dashboard.
 * `fallback` (già tradotto dal chiamante, es. `tc("unexpectedError")`) copre
 * gli errori senza un messaggio del backend (rete, ecc.); di default resta
 * il testo italiano storico per i chiamanti che non ne passano uno. */
export function errorMessage(err: unknown, fallback = "Errore imprevisto."): string {
  return err instanceof ApiClientError ? err.message : fallback;
}

/** `BlogRole` → chiave in `BlogAdmin.roles` (messages/{it,en}.json): le due
 * nomenclature divergono solo per `co_autore`/`co-author`. */
const ROLE_MESSAGE_KEYS: Record<BlogRole, string> = {
  autore: "author",
  co_autore: "co-author",
  revisore: "reviewer",
  mediatore: "mediator",
};

export function roleMessageKey(role: BlogRole): string {
  return ROLE_MESSAGE_KEYS[role] ?? role;
}
