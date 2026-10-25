"use client";

import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  type Simulation,
  type SimulationLinkDatum,
  type SimulationNodeDatum,
} from "d3-force";
import { useEffect, useMemo, useRef, useState } from "react";

import type { GraphEdge, GraphNode } from "@/lib/types";

interface SimNode extends SimulationNodeDatum {
  id: string;
  node: GraphNode;
  r: number;
  degree: number;
}
interface SimLink extends SimulationLinkDatum<SimNode> {
  edge: GraphEdge;
}

export const NODE_COLOR: Record<GraphNode["type"], string> = {
  blog: "var(--color-primary)",
  post: "var(--color-foreground)",
  tag: "var(--color-info)",
  category: "var(--color-warn)",
};
const BASE_RADIUS: Record<GraphNode["type"], number> = { blog: 11, post: 4, tag: 5, category: 6 };
const LINK_DISTANCE: Record<GraphEdge["type"], number> = { in: 36, tag: 64, category: 64, link: 90 };
/** `source`/`target` di un arco: stringa prima che d3 risolva i riferimenti, nodo dopo. */
const endId = (end: string | number | SimNode): string => (typeof end === "object" ? end.id : String(end));

const MIN_ZOOM = 0.25;
const MAX_ZOOM = 5;
const LABEL_ZOOM = 1.8;

interface View {
  x: number;
  y: number;
  k: number;
}

/** Grafo a forze (d3-force) in SVG: pan e zoom con rotella/trascinamento, nodi trascinabili, evidenza dei
 * vicini al passaggio o alla selezione. Il layout si ricalcola solo se cambia l'insieme di nodi/archi; le
 * posizioni si aggiornano direttamente sul DOM a ogni tick (niente re-render React per frame). */
/** Vista che contiene tutti i nodi (con un margine), mai oltre l'ingrandimento 1.8. Il grafo è centrato
 * nell'origine e il gruppo interno è traslato al centro del riquadro. */
function fitView(nodes: SimNode[], { w, h }: { w: number; h: number }): View {
  const pad = 48;
  const xs = nodes.map((n) => n.x ?? 0);
  const ys = nodes.map((n) => n.y ?? 0);
  const [minX, maxX, minY, maxY] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
  const k = Math.min(1.8, Math.max(MIN_ZOOM, Math.min((w - 2 * pad) / Math.max(maxX - minX, 1), (h - 2 * pad) / Math.max(maxY - minY, 1))));
  const cx = (minX + maxX) / 2;
  const cy = (minY + maxY) / 2;
  return { k, x: w / 2 - k * (w / 2 + cx), y: h / 2 - k * (h / 2 + cy) };
}

export function KnowledgeGraph({
  nodes,
  edges,
  selectedId,
  matches,
  onSelect,
  ariaLabel,
  labelOf,
}: {
  nodes: GraphNode[];
  edges: GraphEdge[];
  selectedId: string | null;
  /** nodi che corrispondono alla ricerca: gli altri si attenuano; `null` = nessuna ricerca */
  matches: Set<string> | null;
  onSelect: (id: string | null) => void;
  ariaLabel: string;
  labelOf: (type: GraphNode["type"]) => string;
}) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);
  const nodeEls = useRef(new Map<string, SVGGElement>());
  const linkEls = useRef(new Map<number, SVGLineElement>());
  const simRef = useRef<Simulation<SimNode, SimLink> | null>(null);
  const [size, setSize] = useState({ w: 800, h: 560 });
  const [view, setView] = useState<View>({ x: 0, y: 0, k: 1 });
  const viewRef = useRef(view);
  useEffect(() => {
    viewRef.current = view;
  }, [view]);
  const [hovered, setHovered] = useState<string | null>(null);
  const sizeRef = useRef(size);
  // finché l'utente non sposta o ingrandisce la vista, a layout assestato la mappa si adatta allo spazio disponibile
  const adjusted = useRef(false);

  useEffect(() => {
    sizeRef.current = size;
  }, [size]);

  const { simNodes, simLinks, neighbours } = useMemo(() => {
    const degree = new Map<string, number>();
    const adj = new Map<string, Set<string>>();
    const ids = new Set(nodes.map((n) => n.id));
    const validEdges = edges.filter((e) => ids.has(e.source) && ids.has(e.target));
    for (const e of validEdges) {
      degree.set(e.source, (degree.get(e.source) ?? 0) + 1);
      degree.set(e.target, (degree.get(e.target) ?? 0) + 1);
      if (!adj.has(e.source)) adj.set(e.source, new Set());
      if (!adj.has(e.target)) adj.set(e.target, new Set());
      adj.get(e.source)!.add(e.target);
      adj.get(e.target)!.add(e.source);
    }
    const simNodes: SimNode[] = nodes.map((node, i) => {
      const d = degree.get(node.id) ?? 0;
      // posizione iniziale su una spirale: evita sovrapposizioni esatte e rende il layout ripetibile
      const angle = i * 2.399963;
      const radius = 14 * Math.sqrt(i + 1);
      return { id: node.id, node, degree: d, r: BASE_RADIUS[node.type] + Math.sqrt(d) * 1.6, x: Math.cos(angle) * radius, y: Math.sin(angle) * radius };
    });
    const simLinks: SimLink[] = validEdges.map((edge) => ({ source: edge.source, target: edge.target, edge }));
    return { simNodes, simLinks, neighbours: adj };
  }, [nodes, edges]);

  // dimensioni del contenitore
  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      if (width > 0 && height > 0) setSize({ w: width, h: height });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // simulazione: posizioni scritte direttamente sul DOM
  useEffect(() => {
    const sim = forceSimulation<SimNode>(simNodes)
      .force("link", forceLink<SimNode, SimLink>(simLinks).id((d) => d.id).distance((l) => LINK_DISTANCE[l.edge.type]).strength(0.5))
      .force("charge", forceManyBody<SimNode>().strength((d) => -60 - d.r * 6).distanceMax(320))
      .force("collide", forceCollide<SimNode>().radius((d) => d.r + 3))
      .force("center", forceCenter(0, 0))
      .alphaDecay(0.035);
    sim.on("tick", () => {
      for (const d of simNodes) nodeEls.current.get(d.id)?.setAttribute("transform", `translate(${d.x ?? 0},${d.y ?? 0})`);
      simLinks.forEach((l, i) => {
        const el = linkEls.current.get(i);
        const s = l.source as SimNode;
        const t = l.target as SimNode;
        if (el) {
          el.setAttribute("x1", String(s.x ?? 0));
          el.setAttribute("y1", String(s.y ?? 0));
          el.setAttribute("x2", String(t.x ?? 0));
          el.setAttribute("y2", String(t.y ?? 0));
        }
      });
    });
    sim.on("end", () => {
      if (adjusted.current || simNodes.length === 0) return;
      setView(fitView(simNodes, sizeRef.current));
    });
    adjusted.current = false;
    simRef.current = sim;
    return () => {
      sim.stop();
      simRef.current = null;
    };
  }, [simNodes, simLinks]);

  // rotella = zoom centrato sul puntatore (listener non passivo per poter bloccare lo scroll della pagina)
  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) return;
    function onWheel(e: WheelEvent) {
      e.preventDefault();
      const rect = svg!.getBoundingClientRect();
      const px = e.clientX - rect.left;
      const py = e.clientY - rect.top;
      const v = viewRef.current;
      adjusted.current = true;
      const k = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, v.k * Math.exp(-e.deltaY * 0.0015)));
      setView({ k, x: px - ((px - v.x) / v.k) * k, y: py - ((py - v.y) / v.k) * k });
    }
    svg.addEventListener("wheel", onWheel, { passive: false });
    return () => svg.removeEventListener("wheel", onWheel);
  }, []);

  function toGraph(clientX: number, clientY: number) {
    const rect = svgRef.current!.getBoundingClientRect();
    const v = viewRef.current;
    return { x: (clientX - rect.left - v.x) / v.k, y: (clientY - rect.top - v.y) / v.k };
  }

  function startPan(e: React.PointerEvent<SVGSVGElement>) {
    if (e.target !== e.currentTarget) return;
    onSelect(null);
    adjusted.current = true;
    const start = { x: e.clientX, y: e.clientY, vx: viewRef.current.x, vy: viewRef.current.y };
    const svg = e.currentTarget;
    svg.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => setView((v) => ({ ...v, x: start.vx + ev.clientX - start.x, y: start.vy + ev.clientY - start.y }));
    const up = () => {
      svg.removeEventListener("pointermove", move);
      svg.removeEventListener("pointerup", up);
    };
    svg.addEventListener("pointermove", move);
    svg.addEventListener("pointerup", up);
  }

  function startDrag(e: React.PointerEvent<SVGGElement>, d: SimNode) {
    e.stopPropagation();
    adjusted.current = true;
    const g = e.currentTarget;
    g.setPointerCapture(e.pointerId);
    let moved = false;
    simRef.current?.alphaTarget(0.25).restart();
    const move = (ev: PointerEvent) => {
      moved = true;
      const p = toGraph(ev.clientX, ev.clientY);
      d.fx = p.x;
      d.fy = p.y;
    };
    const up = () => {
      d.fx = null;
      d.fy = null;
      simRef.current?.alphaTarget(0);
      g.removeEventListener("pointermove", move);
      g.removeEventListener("pointerup", up);
      if (!moved) onSelect(d.id);
    };
    g.addEventListener("pointermove", move);
    g.addEventListener("pointerup", up);
  }

  function zoomBy(factor: number) {
    adjusted.current = true;
    setView((v) => {
      const k = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, v.k * factor));
      const cx = size.w / 2;
      const cy = size.h / 2;
      return { k, x: cx - ((cx - v.x) / v.k) * k, y: cy - ((cy - v.y) / v.k) * k };
    });
  }

  // nodi in evidenza: ricerca, altrimenti selezione/passaggio con i loro vicini
  const focusId = hovered ?? selectedId;
  const focusSet = useMemo(() => {
    if (matches) return matches;
    if (!focusId) return null;
    return new Set([focusId, ...(neighbours.get(focusId) ?? [])]);
  }, [matches, focusId, neighbours]);
  const dim = (id: string) => (focusSet && !focusSet.has(id) ? 0.12 : 1);
  const showLabel = (d: SimNode) =>
    d.node.type === "blog" || view.k >= LABEL_ZOOM || d.degree >= 6 || d.id === selectedId || d.id === hovered || (focusSet?.has(d.id) ?? false);

  return (
    <div ref={wrapRef} className="relative h-full min-h-[420px] w-full overflow-hidden rounded-xl border border-border bg-surface">
      <svg
        ref={svgRef}
        role="img"
        aria-label={ariaLabel}
        className="h-full w-full cursor-grab touch-none select-none active:cursor-grabbing"
        viewBox={`0 0 ${size.w} ${size.h}`}
        onPointerDown={startPan}
      >
        <g transform={`translate(${view.x},${view.y}) scale(${view.k})`}>
          <g transform={`translate(${size.w / 2},${size.h / 2})`}>
            {simLinks.map((l, i) => (
              <line
                key={i}
                ref={(el) => {
                  if (el) linkEls.current.set(i, el);
                  else linkEls.current.delete(i);
                }}
                stroke="var(--color-border)"
                strokeWidth={l.edge.type === "link" ? 1.6 : 1}
                strokeDasharray={l.edge.via === "wiki" ? "4 3" : undefined}
                opacity={focusSet && !(focusSet.has(endId(l.source)) && focusSet.has(endId(l.target))) ? 0.08 : l.edge.type === "link" ? 0.9 : 0.55}
                style={l.edge.type === "link" ? { stroke: "var(--color-primary)" } : undefined}
              />
            ))}
            {simNodes.map((d) => (
              <g
                key={d.id}
                ref={(el) => {
                  if (el) nodeEls.current.set(d.id, el);
                  else nodeEls.current.delete(d.id);
                }}
                role="button"
                tabIndex={0}
                aria-label={`${labelOf(d.node.type)}: ${d.node.label}`}
                aria-pressed={d.id === selectedId}
                className="cursor-pointer outline-none"
                opacity={dim(d.id)}
                onPointerDown={(e) => startDrag(e, d)}
                onPointerEnter={() => setHovered(d.id)}
                onPointerLeave={() => setHovered(null)}
                onFocus={() => setHovered(d.id)}
                onBlur={() => setHovered(null)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    onSelect(d.id);
                  }
                }}
              >
                <circle
                  r={d.r}
                  fill={NODE_COLOR[d.node.type]}
                  stroke={d.id === selectedId ? "var(--color-foreground)" : "var(--color-surface)"}
                  strokeWidth={d.id === selectedId ? 2.5 : 1.5}
                />
                {showLabel(d) && (
                  <text
                    y={-d.r - 4}
                    textAnchor="middle"
                    fontSize={11 / Math.max(view.k, 0.6)}
                    fill="var(--color-foreground)"
                    stroke="var(--color-surface)"
                    strokeWidth={3 / Math.max(view.k, 0.6)}
                    paintOrder="stroke"
                    pointerEvents="none"
                  >
                    {d.node.label.length > 40 ? `${d.node.label.slice(0, 39)}…` : d.node.label}
                  </text>
                )}
              </g>
            ))}
          </g>
        </g>
      </svg>
      <div className="absolute bottom-3 right-3 flex flex-col overflow-hidden rounded-lg border border-border bg-background text-foreground shadow-sm">
        {[
          ["+", 1.3],
          ["−", 1 / 1.3],
        ].map(([label, factor]) => (
          <button
            key={String(label)}
            type="button"
            aria-label={label === "+" ? "Zoom +" : "Zoom −"}
            onClick={() => zoomBy(factor as number)}
            className="h-8 w-8 text-lg leading-none hover:bg-surface"
          >
            {label}
          </button>
        ))}
        <button type="button" aria-label="Reset" onClick={() => {
            adjusted.current = false;
            setView(fitView(simNodes, size));
          }} className="h-8 w-8 border-t border-border text-xs hover:bg-surface">
          ⌖
        </button>
      </div>
    </div>
  );
}
