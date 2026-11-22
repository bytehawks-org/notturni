"use client";

import { useLocale, useTranslations } from "next-intl";
import Link from "next/link";
import { useEffect, useMemo, useState } from "react";

import { KnowledgeGraph, NODE_COLOR } from "@/components/graph/KnowledgeGraph";
import { Alert } from "@/components/ui/Alert";
import { Toggle } from "@/components/ui/Controls";
import { Input } from "@/components/ui/Field";
import { FilterChip } from "@/components/ui/Pill";
import { EmptyState, SkeletonRows } from "@/components/ui/States";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { formatDate } from "@/lib/format";
import { PLATFORM_ADMIN_ROLES, type GraphData, type GraphEdge, type GraphNode } from "@/lib/types";

type Layer = "blog" | "tag" | "category" | "link";
const LAYERS: Layer[] = ["blog", "tag", "category", "link"];

/** Mappa dei collegamenti: i contenuti dei propri Spazi come grafo — post, Spazi, tag, categorie e i link
 * tra post (nel testo o `[[pagina]]`). Cliccando un nodo se ne vedono i collegamenti; la ricerca attenua il
 * resto; la vista elenco è l'alternativa accessibile al grafo. Agli amministratori è offerta anche la mappa
 * di tutto ciò che è pubblico sulla piattaforma. */
export default function GraphPage() {
  const { user, authFetch } = useAuth();
  const t = useTranslations("GraphPage");
  const tc = useTranslations("Common");
  const locale = useLocale();
  const isAdmin = !!user && PLATFORM_ADMIN_ROLES.includes(user.platform_role);

  const [scope, setScope] = useState<"mine" | "platform">("mine");
  const [drafts, setDrafts] = useState(false);
  const [layers, setLayers] = useState<Record<Layer, boolean>>({ blog: true, tag: true, category: true, link: true });
  const [onlyConnected, setOnlyConnected] = useState(false);
  const [blogFilter, setBlogFilter] = useState("");
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [view, setView] = useState<"graph" | "list">("graph");
  const [error, setError] = useState<string | null>(null);

  // il risultato porta con sé i parametri con cui è stato chiesto: «in caricamento» = non coincidono con quelli correnti
  const requestKey = `${scope}|${drafts}`;
  const [loaded, setLoaded] = useState<{ key: string; data: GraphData } | null>(null);
  useEffect(() => {
    let cancelled = false;
    authFetch((token) => api.graph.get(token, { scope, drafts }))
      .then((data) => {
        if (!cancelled) {
          setLoaded({ key: requestKey, data });
          setError(null);
        }
      })
      .catch((err) => !cancelled && setError(err instanceof ApiClientError ? err.message : tc("unexpectedError")));
    return () => {
      cancelled = true;
    };
  }, [authFetch, scope, drafts, requestKey, tc]);
  const data = loaded && loaded.key === requestKey ? loaded.data : null;

  const blogs = useMemo(() => (data?.nodes ?? []).filter((n) => n.type === "blog"), [data]);

  // filtri lato client: Spazio, livelli, solo connessi
  const { nodes, edges } = useMemo(() => {
    if (!data) return { nodes: [] as GraphNode[], edges: [] as GraphEdge[] };
    const keepPost = (n: GraphNode) => !blogFilter || n.blog === blogFilter;
    let ns = data.nodes.filter((n) => {
      if (n.type === "post") return keepPost(n);
      if (n.type === "blog") return layers.blog && (!blogFilter || n.slug === blogFilter);
      return layers[n.type];
    });
    const ids = new Set(ns.map((n) => n.id));
    let es = data.edges.filter((e) => ids.has(e.source) && ids.has(e.target) && (e.type !== "link" || layers.link));
    // tag e categorie che non collegano più nulla scompaiono
    const used = new Set(es.flatMap((e) => [e.source, e.target]));
    ns = ns.filter((n) => (n.type === "tag" || n.type === "category" ? used.has(n.id) : true));
    if (onlyConnected) {
      const connected = new Set(es.filter((e) => e.type !== "in").flatMap((e) => [e.source, e.target]));
      ns = ns.filter((n) => n.type === "blog" || connected.has(n.id));
      const keep = new Set(ns.map((n) => n.id));
      es = es.filter((e) => keep.has(e.source) && keep.has(e.target));
    }
    return { nodes: ns, edges: es };
  }, [data, blogFilter, layers, onlyConnected]);

  const byId = useMemo(() => new Map(nodes.map((n) => [n.id, n])), [nodes]);
  const matches = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return null;
    return new Set(nodes.filter((n) => n.label.toLowerCase().includes(q)).map((n) => n.id));
  }, [query, nodes]);
  const degree = useMemo(() => {
    const d = new Map<string, number>();
    for (const e of edges) {
      d.set(e.source, (d.get(e.source) ?? 0) + 1);
      d.set(e.target, (d.get(e.target) ?? 0) + 1);
    }
    return d;
  }, [edges]);
  const selected = selectedId ? (byId.get(selectedId) ?? null) : null;
  const connections = useMemo(() => {
    if (!selected) return [];
    return edges
      .filter((e) => e.source === selected.id || e.target === selected.id)
      .map((e) => ({ edge: e, other: byId.get(e.source === selected.id ? e.target : e.source) }))
      .filter((c): c is { edge: GraphEdge; other: GraphNode } => !!c.other);
  }, [selected, edges, byId]);

  const typeLabel = (type: GraphNode["type"]) => t(`types.${type}`);

  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h1 className="font-serif text-2xl text-foreground">{t("title")}</h1>
          <p className="mt-1 text-sm text-muted">{t("intro")}</p>
        </div>
        <div className="flex items-center gap-2">
          <FilterChip active={view === "graph"} onClick={() => setView("graph")}>
            {t("viewGraph")}
          </FilterChip>
          <FilterChip active={view === "list"} onClick={() => setView("list")}>
            {t("viewList")}
          </FilterChip>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        {LAYERS.map((layer) => (
          <FilterChip key={layer} active={layers[layer]} onClick={() => setLayers((l) => ({ ...l, [layer]: !l[layer] }))}>
            <span aria-hidden="true" className="mr-1.5 inline-block h-2 w-2 rounded-full align-middle" style={{ background: layer === "link" ? "var(--color-primary)" : NODE_COLOR[layer] }} />
            {t(`layers.${layer}`)}
          </FilterChip>
        ))}
        <select
          aria-label={t("space")}
          value={blogFilter}
          onChange={(e) => setBlogFilter(e.target.value)}
          className="rounded-lg border border-border bg-surface px-2.5 py-1.5 text-xs text-foreground"
        >
          <option value="">{t("allSpaces")}</option>
          {blogs.map((b) => (
            <option key={b.id} value={b.slug}>
              {b.label}
            </option>
          ))}
        </select>
        <Input
          type="search"
          aria-label={t("search")}
          placeholder={t("search")}
          className="max-w-[220px] py-1.5 text-xs"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        {matches && <span className="text-xs text-muted">{t("matches", { count: matches.size })}</span>}
      </div>
      <div className="flex flex-wrap gap-x-6 gap-y-2 rounded-xl border border-border bg-surface px-4 py-3">
        <div className="min-w-[220px] flex-1">
          <Toggle checked={drafts} onChange={setDrafts} label={t("drafts")} disabled={scope === "platform"} />
        </div>
        <div className="min-w-[220px] flex-1">
          <Toggle checked={onlyConnected} onChange={setOnlyConnected} label={t("onlyConnected")} />
        </div>
        {isAdmin && (
          <div className="min-w-[220px] flex-1">
            <Toggle checked={scope === "platform"} onChange={(v) => setScope(v ? "platform" : "mine")} label={t("wholePlatform")} />
          </div>
        )}
      </div>

      {error && <Alert kind="error">{error}</Alert>}
      {data?.truncated && <Alert kind="info">{t("truncated", { count: data.nodes.filter((n) => n.type === "post").length })}</Alert>}

      {!data && !error ? (
        <SkeletonRows rows={6} />
      ) : data && data.nodes.length === 0 ? (
        <EmptyState title={t("emptyTitle")} body={t("emptyBody")} />
      ) : data ? (
        <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_300px]">
          {view === "graph" ? (
            <div className="h-[68vh] min-h-[420px]">
              <KnowledgeGraph
                nodes={nodes}
                edges={edges}
                selectedId={selected?.id ?? null}
                matches={matches}
                onSelect={setSelectedId}
                ariaLabel={t("graphLabel", { nodes: nodes.length, edges: edges.length })}
                labelOf={typeLabel}
              />
            </div>
          ) : (
            <ul className="flex max-h-[68vh] flex-col divide-y divide-border overflow-y-auto rounded-xl border border-border bg-surface">
              {[...nodes]
                .filter((n) => !matches || matches.has(n.id))
                .sort((a, b) => (degree.get(b.id) ?? 0) - (degree.get(a.id) ?? 0) || a.label.localeCompare(b.label))
                .map((n) => (
                  <li key={n.id}>
                    <button
                      type="button"
                      onClick={() => setSelectedId(n.id)}
                      aria-pressed={n.id === selected?.id}
                      className={`flex w-full items-center gap-3 px-4 py-2.5 text-left text-sm hover:bg-background ${n.id === selected?.id ? "bg-background" : ""}`}
                    >
                      <span aria-hidden="true" className="h-2.5 w-2.5 shrink-0 rounded-full" style={{ background: NODE_COLOR[n.type] }} />
                      <span className="min-w-0 flex-1 truncate text-foreground">{n.label}</span>
                      <span className="shrink-0 text-xs text-muted">{typeLabel(n.type)}</span>
                      <span className="w-8 shrink-0 text-right text-xs tabular-nums text-muted">{degree.get(n.id) ?? 0}</span>
                    </button>
                  </li>
                ))}
            </ul>
          )}

          <aside className="flex flex-col gap-3 rounded-xl border border-border bg-surface p-4 text-sm" aria-live="polite">
            {selected ? (
              <>
                <div>
                  <span className="text-[11px] uppercase tracking-[.08em] text-muted">{typeLabel(selected.type)}</span>
                  <h2 className="font-serif text-lg leading-snug text-foreground">{selected.label}</h2>
                  {selected.type === "post" && (
                    <p className="mt-1 text-[12px] text-muted">
                      {selected.blog}
                      {selected.published_at ? ` · ${formatDate(selected.published_at, locale)}` : ` · ${t("draft")}`}
                    </p>
                  )}
                </div>
                <div className="flex flex-wrap gap-3 text-[13px]">
                  {selected.type === "post" && selected.path && selected.published_at && (
                    <Link href={selected.path} className="text-primary no-underline hover:underline" target="_blank">
                      {t("open")} ↗
                    </Link>
                  )}
                  {selected.type === "post" && selected.blog && (
                    <Link href={`/dashboard/blogs/${selected.blog}/posts/${selected.id.slice(5)}`} className="text-primary no-underline hover:underline">
                      {t("edit")}
                    </Link>
                  )}
                  {selected.type === "blog" && (
                    <>
                      <Link href={`/${selected.slug}`} className="text-primary no-underline hover:underline" target="_blank">
                        {t("open")} ↗
                      </Link>
                      <Link href={`/dashboard/blogs/${selected.slug}`} className="text-primary no-underline hover:underline">
                        {t("manage")}
                      </Link>
                    </>
                  )}
                  {selected.type === "tag" && (
                    <Link href={`/tag/${selected.tag}`} className="text-primary no-underline hover:underline" target="_blank">
                      {t("open")} ↗
                    </Link>
                  )}
                </div>
                <div>
                  <h3 className="mb-1 text-[11px] uppercase tracking-[.08em] text-muted">{t("connections", { count: connections.length })}</h3>
                  {connections.length === 0 ? (
                    <p className="text-muted">{t("noConnections")}</p>
                  ) : (
                    <ul className="flex max-h-[34vh] flex-col gap-0.5 overflow-y-auto">
                      {connections.map(({ edge, other }, i) => (
                        <li key={i}>
                          <button type="button" onClick={() => setSelectedId(other.id)} className="flex w-full items-center gap-2 rounded px-1.5 py-1 text-left hover:bg-background">
                            <span aria-hidden="true" className="h-2 w-2 shrink-0 rounded-full" style={{ background: NODE_COLOR[other.type] }} />
                            <span className="min-w-0 flex-1 truncate text-foreground">{other.label}</span>
                            <span className="shrink-0 text-[11px] text-muted">
                              {edge.type === "link" ? (edge.source === selected.id ? t("linksTo") : t("linkedFrom")) : typeLabel(other.type)}
                            </span>
                          </button>
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              </>
            ) : (
              <p className="text-muted">{t("pick")}</p>
            )}
            <div className="mt-auto border-t border-border pt-3 text-[12px] text-muted">
              {t("counts", { nodes: nodes.length, edges: edges.length })}
              <ul className="mt-2 flex flex-wrap gap-x-3 gap-y-1">
                {(["blog", "post", "tag", "category"] as const).map((type) => (
                  <li key={type} className="flex items-center gap-1.5">
                    <span aria-hidden="true" className="h-2 w-2 rounded-full" style={{ background: NODE_COLOR[type] }} />
                    {typeLabel(type)}
                  </li>
                ))}
                <li className="flex items-center gap-1.5">
                  <span aria-hidden="true" className="h-0.5 w-4 bg-primary" />
                  {t("layers.link")}
                </li>
              </ul>
            </div>
          </aside>
        </div>
      ) : null}
    </div>
  );
}
