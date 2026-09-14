"use client";

import { useLocale, useTranslations } from "next-intl";
import { useCallback, useEffect, useState, type FormEvent } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { Input, Label } from "@/components/ui/Field";
import { NoteDialog } from "@/components/ui/NoteDialog";
import { FilterChip, Pill } from "@/components/ui/Pill";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { useToast } from "@/components/ui/Toast";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import type { GdprRequest, GdprRequestStatus, GdprRequestType } from "@/lib/types";

const FILTERS: ("all" | GdprRequestStatus)[] = ["all", "open", "approved", "completed", "rejected"];

/** Coda richieste GDPR (mockup 5f): scadenza legale, export automatico,
 * cancellazioni con seconda approvazione. */
export default function GdprRequestsPage() {
  const { user, authFetch } = useAuth();
  const t = useTranslations("Gdpr");
  const tc = useTranslations("Common");
  const locale = useLocale();
  const notify = useToast();
  const [filter, setFilter] = useState<(typeof FILTERS)[number]>("all");
  const [rows, setRows] = useState<GdprRequest[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [username, setUsername] = useState("");
  const [type, setType] = useState<GdprRequestType>("export");
  const [note, setNote] = useState("");
  const [rejecting, setRejecting] = useState<GdprRequest | null>(null);
  const [busy, setBusy] = useState(false);
  // istante di riferimento per le scadenze, calcolato una volta al montaggio
  const [now] = useState(() => Date.now());

  const errorMessage = useCallback((err: unknown) => (err instanceof ApiClientError ? err.message : tc("unexpectedError")), [tc]);
  const load = useCallback(() => {
    authFetch((token) => api.admin.listGdpr(token, filter === "all" ? undefined : filter))
      .then(setRows)
      .catch((err) => setError(errorMessage(err)));
  }, [authFetch, filter, errorMessage]);
  useEffect(load, [load]);

  async function act(fn: () => Promise<unknown>, done: string) {
    setBusy(true);
    setError(null);
    try {
      await fn();
      notify(done);
      load();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function execute(r: GdprRequest) {
    await act(async () => {
      const data = await authFetch((token) => api.admin.executeGdpr(token, r.id));
      if (r.type === "export") {
        const url = URL.createObjectURL(new Blob([JSON.stringify(data, null, 2)], { type: "application/json" }));
        const a = document.createElement("a");
        a.href = url;
        a.download = `notturni-gdpr-${r.username}.json`;
        a.click();
        URL.revokeObjectURL(url);
      }
    }, r.type === "export" ? t("exportDone") : t("deletionDone"));
  }

  const overdue = (r: GdprRequest) => (r.status === "open" || r.status === "approved") && new Date(r.deadline_at).getTime() < now;
  const tone = (s: GdprRequestStatus): "ok" | "danger" | "info" | "warn" =>
    s === "completed" ? "ok" : s === "rejected" ? "danger" : s === "approved" ? "info" : "warn";

  return (
    <div className="mx-auto flex max-w-6xl flex-col gap-6">
      <div className="flex flex-col gap-1">
        <h1 className="font-serif text-[28px] font-medium leading-tight text-foreground">{t("title")}</h1>
        <p className="text-sm text-muted">{t("subtitle")}</p>
      </div>
      {error && <Alert kind="error">{error}</Alert>}

      <Card className="flex flex-col gap-3">
        <CardTitle>{t("newRequest")}</CardTitle>
        <form
          className="grid gap-3 md:grid-cols-[1fr_auto_2fr_auto] md:items-end"
          onSubmit={(e: FormEvent) => {
            e.preventDefault();
            void act(() => authFetch((token) => api.admin.createGdpr(token, { username: username.trim(), type, note })), t("created")).then(() => {
              setUsername("");
              setNote("");
            });
          }}
        >
          <div>
            <Label htmlFor="gdpr-user">{t("username")}</Label>
            <Input id="gdpr-user" required value={username} onChange={(e) => setUsername(e.target.value)} placeholder="username" />
          </div>
          <div>
            <Label htmlFor="gdpr-type">{t("col.type")}</Label>
            <select id="gdpr-type" value={type} onChange={(e) => setType(e.target.value as GdprRequestType)} className="rounded-lg border border-border bg-surface px-3 py-2.5 text-sm text-foreground">
              <option value="export">{t("type.export")}</option>
              <option value="deletion">{t("type.deletion")}</option>
            </select>
          </div>
          <div>
            <Label htmlFor="gdpr-note">{t("note")}</Label>
            <Input id="gdpr-note" required value={note} onChange={(e) => setNote(e.target.value)} placeholder={t("notePlaceholder")} />
          </div>
          <Button type="submit" disabled={busy}>
            {tc("add")}
          </Button>
        </form>
        <span className="text-[13px] text-muted">{t("newRequestNote")}</span>
      </Card>

      <div className="flex flex-wrap gap-1.5">
        {FILTERS.map((f) => (
          <FilterChip key={f} active={filter === f} onClick={() => setFilter(f)}>
            {t(`filter.${f}`)}
          </FilterChip>
        ))}
      </div>
      {rows === null && !error && <SkeletonRows rows={4} />}
      {rows !== null && rows.length === 0 && <EmptyState glyph="⚖" title={t("emptyTitle")} body={t("emptyBody")} />}
      {rows && rows.length > 0 && (
        <div className="overflow-x-auto rounded-xl border border-border bg-surface">
          <table className="w-full text-left text-sm">
            <thead className="border-b border-border font-mono text-[11px] uppercase tracking-[.06em] text-muted">
              <tr>
                <th className="px-4 py-3">{t("col.request")}</th>
                <th className="px-4 py-3">{t("col.type")}</th>
                <th className="px-4 py-3">{t("col.received")}</th>
                <th className="px-4 py-3">{t("col.deadline")}</th>
                <th className="px-4 py-3">{t("col.status")}</th>
                <th className="px-4 py-3" />
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr key={r.id} className="border-b border-border align-top last:border-0">
                  <td className="px-4 py-3">
                    <div className="flex flex-col">
                      <span className="font-medium text-foreground">@{r.username}</span>
                      {r.note && <span className="text-xs text-muted">{r.note}</span>}
                      {r.created_by_username && <span className="text-xs text-muted">{t("createdBy", { username: r.created_by_username })}</span>}
                      {r.approved_by_username && <span className="text-xs text-muted">{t("approvedBy", { username: r.approved_by_username })}</span>}
                    </div>
                  </td>
                  <td className="px-4 py-3 text-muted">{t(`type.${r.type}`)}</td>
                  <td className="whitespace-nowrap px-4 py-3 text-muted">{formatDate(r.created_at, locale, { day: "numeric", month: "short" })}</td>
                  <td className={`whitespace-nowrap px-4 py-3 ${overdue(r) ? "font-semibold text-danger" : "text-muted"}`}>
                    {formatDate(r.deadline_at, locale, { day: "numeric", month: "short" })}
                    {overdue(r) && ` · ${t("overdue")}`}
                  </td>
                  <td className="px-4 py-3">
                    <Pill tone={tone(r.status)}>{t(`status.${r.status}`)}</Pill>
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex flex-wrap justify-end gap-2">
                      {r.status === "open" && r.type === "deletion" && r.created_by_username !== user?.username && (
                        <Button size="sm" variant="secondary" disabled={busy} onClick={() => act(() => authFetch((tk) => api.admin.approveGdpr(tk, r.id)), t("approved"))}>
                          {t("approve")}
                        </Button>
                      )}
                      {((r.status === "open" && r.type === "export") || r.status === "approved") && (
                        <Button size="sm" variant={r.type === "deletion" ? "danger" : "primary"} disabled={busy} onClick={() => execute(r)}>
                          {r.type === "export" ? t("runExport") : t("runDeletion")}
                        </Button>
                      )}
                      {(r.status === "open" || r.status === "approved") && (
                        <Button size="sm" variant="ghost" disabled={busy} onClick={() => setRejecting(r)}>
                          {t("reject")}
                        </Button>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <NoteDialog
        open={rejecting !== null}
        title={rejecting ? t("rejectTitle", { username: rejecting.username }) : ""}
        confirmLabel={t("reject")}
        danger
        onCancel={() => setRejecting(null)}
        onConfirm={(reason) => {
          const r = rejecting;
          setRejecting(null);
          if (r) void act(() => authFetch((tk) => api.admin.rejectGdpr(tk, r.id, reason)), t("rejected"));
        }}
      />
    </div>
  );
}
