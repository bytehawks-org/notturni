import { ApiClientError } from "@/lib/api";

/** Messaggio d'errore leggibile dai vari tab della scheda blog del dashboard. */
export function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

export const ROLE_LABELS: Record<string, string> = {
  autore: "Autore",
  co_autore: "Co-autore",
  revisore: "Revisore",
  mediatore: "Mediatore",
};
