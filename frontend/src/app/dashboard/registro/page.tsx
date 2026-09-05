"use client";

import { useCallback, useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import {
  AUDIT_ACTION_LABELS,
  AUDIT_ACTOR_TYPE_LABELS,
  type AuditLogEntry,
} from "@/lib/types";

const PAGE_SIZE = 50;

function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

function actionLabel(action: string): string {
  return AUDIT_ACTION_LABELS[action] ?? action;
}

function targetLabel(entry: AuditLogEntry): string {
  if (!entry.target_type) return "—";
  return entry.target_id ? `${entry.target_type} · ${entry.target_id.slice(0, 8)}` : entry.target_type;
}

function detailsLabel(payload: Record<string, unknown>): string {
  return Object.keys(payload).length ? JSON.stringify(payload) : "—";
}

export default function DashboardAuditLogPage() {
  const { authFetch } = useAuth();
  const [action, setAction] = useState("");
  const [entries, setEntries] = useState<AuditLogEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [hasMore, setHasMore] = useState(false);

  const fetchPage = useCallback(
    (offset: number) =>
      authFetch((token) =>
        api.admin.listAuditLog(token, {
          action: action || undefined,
          limit: String(PAGE_SIZE),
          offset: String(offset),
        })
      ),
    [authFetch, action]
  );

  // (ri)carica dalla prima pagina; scatta al cambio di filtro
  const load = useCallback(() => {
    fetchPage(0)
      .then((batch) => {
        setEntries(batch);
        setHasMore(batch.length === PAGE_SIZE);
        setError(null);
      })
      .catch((err) => setError(errorMessage(err)));
  }, [fetchPage]);

  useEffect(load, [load]);

  function loadMore() {
    fetchPage(entries?.length ?? 0)
      .then((batch) => {
        setEntries((prev) => [...(prev ?? []), ...batch]);
        setHasMore(batch.length === PAGE_SIZE);
      })
      .catch((err) => setError(errorMessage(err)));
  }

  return (
    <div>
      <div className="mb-2 flex items-center justify-between">
        <h1 className="font-serif text-2xl text-foreground">Registro di audit</h1>
        <select
          value={action}
          onChange={(e) => setAction(e.target.value)}
          aria-label="Filtra per azione"
          className="max-w-xs rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-primary/40"
        >
          <option value="">Tutte le azioni</option>
          {Object.keys(AUDIT_ACTION_LABELS).map((key) => (
            <option key={key} value={key}>
              {AUDIT_ACTION_LABELS[key]}
            </option>
          ))}
        </select>
      </div>
      <p className="mb-6 text-sm text-muted">
        Azioni sensibili degli ultimi mesi. Gli eventi più vecchi della retention sono
        archiviati su storage e non compaiono qui.
      </p>
      {error && <Alert kind="error">{error}</Alert>}

      <div className="overflow-x-auto rounded-lg border border-border">
        <table className="w-full text-left text-sm">
          <thead className="border-b border-border text-muted">
            <tr>
              <th className="px-4 py-3 whitespace-nowrap">Quando</th>
              <th className="px-4 py-3">Azione</th>
              <th className="px-4 py-3">Attore</th>
              <th className="px-4 py-3">Oggetto</th>
              <th className="px-4 py-3 whitespace-nowrap">IP</th>
              <th className="px-4 py-3">Dettagli</th>
            </tr>
          </thead>
          <tbody>
            {entries?.map((entry) => (
              <tr key={entry.id} className="border-b border-border align-top last:border-0">
                <td className="px-4 py-3 whitespace-nowrap text-muted">
                  {new Date(entry.occurred_at).toLocaleString("it-IT")}
                </td>
                <td className="px-4 py-3 text-foreground">{actionLabel(entry.action)}</td>
                <td className="px-4 py-3 text-muted">
                  {entry.actor_label ?? AUDIT_ACTOR_TYPE_LABELS[entry.actor_type]}
                </td>
                <td className="px-4 py-3 text-muted">{targetLabel(entry)}</td>
                <td className="px-4 py-3 whitespace-nowrap text-muted">{entry.ip ?? "—"}</td>
                <td className="px-4 py-3 font-mono text-xs text-muted break-all">
                  {detailsLabel(entry.payload)}
                </td>
              </tr>
            ))}
            {entries !== null && entries.length === 0 && (
              <tr>
                <td colSpan={6} className="px-4 py-6 text-center text-muted">
                  Nessun evento registrato.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {hasMore && (
        <div className="mt-4 flex justify-center">
          <Button variant="secondary" onClick={loadMore}>
            Carica altri
          </Button>
        </div>
      )}
    </div>
  );
}
