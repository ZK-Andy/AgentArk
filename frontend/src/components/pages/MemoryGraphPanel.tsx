import {
  Alert,
  Box,
  Button,
  Chip,
  Divider,
  FormControlLabel,
  IconButton,
  Stack,
  Switch,
  ToggleButton,
  ToggleButtonGroup,
  Tooltip,
  Typography,
} from "@mui/material";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { forceCollide, forceManyBody, forceX, forceY } from "d3-force";
import { Check, RefreshCw, Search, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import ForceGraph2D, {
  type ForceGraphMethods,
  type LinkObject,
  type NodeObject,
} from "react-force-graph-2d";
import { api } from "../../api/client";
import { humanizeMachineLabel } from "../../lib/displayLabels";
import {
  buildMemoryGraphQuery,
  memoryGraphEdgeLabel,
  memoryGraphEdgeTone,
  memoryGraphVisibleSummary,
  type MemoryGraphEdge,
  type MemoryGraphMode,
  type MemoryGraphNode,
  type MemoryGraphPayload,
} from "./memoryGraph";
import { asRecord, errMessage, num, str, type JsonRecord } from "./pageHelpers";

const GRAPH_MEMORY_STATUSES = ["active", "stale", "deprecated"];
const GRAPH_RELATION_STATUSES = ["candidate", "confirmed"];

const TWO_PI = Math.PI * 2;

const MONO_FONT = '"JetBrains Mono", ui-monospace, monospace';
const ACCENT = "#78f2b0";

// Node fields we derive once at merge time and stash on the simulation object so
// the per-frame paint never recomputes them (no string building per frame).
type MemoryNodeExtra = MemoryGraphNode & {
  __color?: string;
  __rim?: string;
  __ring?: string;
  __r?: number;
  __label?: string;
  __degree?: number;
};

type MemoryEdgeExtra = MemoryGraphEdge & {
  __tone?: "semantic" | "supersedes" | "evidence" | "relation" | "explicit";
};

// The simulation augments these objects in place with x/y/vx/vy/fx/fy.
type GNode = NodeObject<MemoryNodeExtra>;
type GLink = LinkObject<MemoryNodeExtra, MemoryEdgeExtra>;

type MemoryGraphPanelProps = {
  focusMemoryId?: string | null;
};

function clamp(value: number, lo: number, hi: number): number {
  return Math.max(lo, Math.min(hi, value));
}

// A link endpoint is an id string before the first tick, a resolved node object after.
function idOf(endpoint: unknown): string {
  if (endpoint && typeof endpoint === "object") {
    const id = (endpoint as { id?: string | number }).id;
    return id == null ? "" : String(id);
  }
  return endpoint == null ? "" : String(endpoint);
}

function nodeMarkSize(node: MemoryGraphNode, degree: number): number {
  const type = str(node.node_type, "memory");
  const base = type === "entity" ? 4.8 : type === "source" ? 3.2 : 3.5;
  const degreeGain = Math.sqrt(Math.max(0, degree)) * 1.18;
  const pinnedGain = node.pinned ? 0.9 : 0;
  const max = type === "entity" ? 11 : 8.8;
  return clamp(base + degreeGain + pinnedGain, 3.1, max);
}

// Palette tuned to the app's mint-on-black token system (see 00-foundation.css):
// muted technical markers instead of saturated chart primaries.
// Shared by the paint code and the legend so they cannot drift.
const CATEGORY_COLORS: Record<string, string> = {
  assistant_preference: "#ff9ec7",
  work_preference: "#549bf0",
  project_domain_memory: "#ffbe63",
  ephemeral_context: "#f5e08a",
  knowledge: "#b7a7ff",
  other: "#ffab7a",
};

const CATEGORY_SHORT: Record<string, string> = {
  assistant_preference: "assistant",
  work_preference: "work",
  project_domain_memory: "project",
  ephemeral_context: "ephemeral",
  knowledge: "knowledge",
  other: "other",
};

const MEMORY_DEFAULT = "#8fc6ff";
const PINNED_COLOR = "#ff9b9b";
const ENTITY_COLOR = ACCENT;
const SOURCE_COLOR = "#93a59c";

function graphNodeColor(node: MemoryGraphNode): string {
  if (node.pinned) return PINNED_COLOR;
  if (node.node_type === "entity") return ENTITY_COLOR;
  if (node.node_type === "source") return SOURCE_COLOR;
  return CATEGORY_COLORS[str(node.category, "")] ?? MEMORY_DEFAULT;
}

function hexChannels(hex: string): [number, number, number] {
  const value = Number.parseInt(hex.slice(1), 16);
  return [(value >> 16) & 0xff, (value >> 8) & 0xff, value & 0xff];
}

function withAlpha(hex: string, alpha: number): string {
  const [r, g, b] = hexChannels(hex);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

function lighten(hex: string, amount: number): string {
  const [r, g, b] = hexChannels(hex);
  const mix = (channel: number) => Math.round(channel + (255 - channel) * amount);
  return `rgb(${mix(r)}, ${mix(g)}, ${mix(b)})`;
}

// knowledge_relation gets its own tone so confirmed relations read as the
// graph's mint "structure"; everything else follows memoryGraphEdgeTone.
function edgeTone(edge: MemoryGraphEdge): MemoryEdgeExtra["__tone"] {
  if (edge.edge_type === "knowledge_relation") return "relation";
  return memoryGraphEdgeTone(edge);
}

const EDGE_HOT: Record<string, string> = {
  semantic: "rgba(245, 224, 138, 0.78)",
  supersedes: "rgba(255, 155, 155, 0.84)",
  evidence: "rgba(190, 205, 197, 0.68)",
  relation: "rgba(120, 242, 176, 0.82)",
  explicit: "rgba(163, 210, 255, 0.72)",
};

const LINK_BASE: Record<string, string> = {
  semantic: "rgba(200, 186, 126, 0.08)",
  supersedes: "rgba(255, 155, 155, 0.18)",
  evidence: "rgba(154, 170, 162, 0.1)",
  relation: "rgba(120, 242, 176, 0.18)",
  explicit: "rgba(143, 198, 255, 0.12)",
};

const EDGE_DIMMED = "rgba(140, 165, 150, 0.035)";

function traceHex(ctx: CanvasRenderingContext2D, r: number): void {
  ctx.beginPath();
  for (let i = 0; i < 6; i += 1) {
    const angle = Math.PI / 6 + i * (Math.PI / 3);
    const x = Math.cos(angle) * r;
    const y = Math.sin(angle) * r;
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  }
  ctx.closePath();
}

function traceDiamond(ctx: CanvasRenderingContext2D, r: number): void {
  ctx.beginPath();
  ctx.moveTo(0, -r);
  ctx.lineTo(r * 0.82, 0);
  ctx.lineTo(0, r);
  ctx.lineTo(-r * 0.82, 0);
  ctx.closePath();
}

function traceSlate(ctx: CanvasRenderingContext2D, r: number): void {
  const w = r * 1.7;
  const h = r * 1.05;
  ctx.beginPath();
  ctx.rect(-w / 2, -h / 2, w, h);
}

function traceNodeMark(ctx: CanvasRenderingContext2D, node: MemoryGraphNode, r: number): void {
  const type = str(node.node_type, "memory");
  if (type === "entity") traceHex(ctx, r);
  else if (type === "source") traceSlate(ctx, r);
  else traceDiamond(ctx, r);
}

function graphTooltip(params: { dataType?: string; data?: unknown }): string {
  const data = asRecord(params.data);
  if (params.dataType === "edge") {
    return [
      `<strong>${memoryGraphEdgeLabel(data as MemoryGraphEdge)}</strong>`,
      str(data.detail, ""),
      str(data.edge_type, ""),
    ]
      .filter(Boolean)
      .join("<br/>");
  }
  return [
    `<strong>${str(data.name || data.label, "Node")}</strong>`,
    humanizeMachineLabel(str(data.node_type, "memory"), "Memory"),
    str(data.value || data.detail, ""),
  ]
    .filter(Boolean)
    .join("<br/>");
}

function inspectorEvidence(edge: MemoryGraphEdge): JsonRecord[] {
  const metadata = asRecord(edge.metadata);
  const values = Array.isArray(metadata.evidence) ? metadata.evidence : [];
  return values.map(asRecord).filter((item) => Object.keys(item).length > 0);
}


export default function MemoryGraphPanel({ focusMemoryId }: MemoryGraphPanelProps) {
  const queryClient = useQueryClient();
  const [mode, setMode] = useState<MemoryGraphMode>(focusMemoryId ? "focus" : "map");
  const [focusId, setFocusId] = useState(focusMemoryId || "");
  const [includeSemantic, setIncludeSemantic] = useState(true);
  const [selectedNode, setSelectedNode] = useState<MemoryGraphNode | null>(null);
  const [selectedEdge, setSelectedEdge] = useState<MemoryGraphEdge | null>(null);

  useEffect(() => {
    if (!focusMemoryId) return;
    setFocusId(focusMemoryId);
    setMode("focus");
  }, [focusMemoryId]);

  const queryPath = useMemo(
    () =>
      buildMemoryGraphQuery({
        mode,
        memoryId: focusId,
        limit: 160,
        statuses: GRAPH_MEMORY_STATUSES,
        relationStatuses: GRAPH_RELATION_STATUSES,
        includeSemantic,
        semanticThreshold: 0.78,
      }),
    [
      focusId,
      includeSemantic,
      mode,
    ],
  );

  const graphQ = useQuery({
    queryKey: ["arkmemory-graph", queryPath],
    queryFn: () => api.rawGet(queryPath) as Promise<MemoryGraphPayload>,
    enabled: mode === "map" || focusId.trim().length > 0,
    staleTime: 15_000,
  });

  const relationStatusMutation = useMutation({
    mutationFn: ({ id, action }: { id: string; action: "confirm" | "reject" }) =>
      api.rawPost(
        `/arkmemory/knowledge-graph/relations/${encodeURIComponent(id)}/${action}`,
      ),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["arkmemory-graph"] });
    },
  });

  const payload = (graphQ.data || {}) as MemoryGraphPayload;
  const nodes = payload.nodes || [];
  const edges = payload.edges || [];
  const summary = memoryGraphVisibleSummary(payload);

  // One legend entry per memory category actually present (unknown categories
  // fold into "memory"), plus pinned/entity/source buckets.
  const legendEntries = useMemo(() => {
    const categoryCounts = new Map<string, number>();
    let entity = 0;
    let source = 0;
    let pinned = 0;
    for (const node of nodes) {
      const type = str(node.node_type, "memory");
      if (type === "entity") entity += 1;
      else if (type === "source") source += 1;
      else if (node.pinned) pinned += 1;
      else {
        const category = str(node.category, "");
        const key = CATEGORY_COLORS[category] ? category : "";
        categoryCounts.set(key, (categoryCounts.get(key) ?? 0) + 1);
      }
    }
    const entries: Array<{ label: string; color: string; count: number }> = [];
    for (const [category, count] of categoryCounts) {
      entries.push({
        label: CATEGORY_SHORT[category] ?? "memory",
        color: CATEGORY_COLORS[category] ?? MEMORY_DEFAULT,
        count,
      });
    }
    entries.sort((a, b) => b.count - a.count);
    if (pinned > 0) entries.push({ label: "pinned", color: PINNED_COLOR, count: pinned });
    if (entity > 0) entries.push({ label: "entity", color: ENTITY_COLOR, count: entity });
    if (source > 0) entries.push({ label: "source", color: SOURCE_COLOR, count: source });
    return entries;
  }, [nodes]);

  // --- Force-graph plumbing (refs so hover/selection never trigger React re-renders) ---
  const fgRef = useRef<ForceGraphMethods<GNode, GLink> | undefined>(undefined);
  const observerRef = useRef<ResizeObserver | null>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });

  const nodesByIdRef = useRef<Map<string, GNode>>(new Map());
  const linksByKeyRef = useRef<Map<string, GLink>>(new Map());
  const neighborsRef = useRef<Map<string, Set<string>>>(new Map());
  const linksByNodeRef = useRef<Map<string, Set<GLink>>>(new Map());
  const hoverRef = useRef<string | null>(null);
  const selectedIdRef = useRef<string | null>(null);
  const highlightNodesRef = useRef<Set<string>>(new Set());
  const highlightLinksRef = useRef<Set<GLink>>(new Set());
  const didFitRef = useRef(false);
  const prevIdsKeyRef = useRef("");
  const prevEdgeKeyRef = useRef("");
  const lastGraphDataRef = useRef<{ nodes: GNode[]; links: GLink[] } | null>(null);

  // Keep the paint callback's view of the selection current without re-creating it.
  useEffect(() => {
    selectedIdRef.current = selectedNode?.id ?? null;
  }, [selectedNode]);

  // Build graphData by MERGING into the existing node objects: the engine mutates
  // x/y/vx/vy in place and diffs by identity, so reusing objects keeps positions
  // stable across refetches instead of re-laying-out the whole graph every poll.
  const graphData = useMemo(() => {
    const byId = nodesByIdRef.current;
    const incomingIds = new Set<string>();

    const degree = new Map<string, number>();
    const links: GLink[] = [];
    // Links get the same identity-preserving merge as nodes: the engine
    // resolves source/target to node objects in place, and the field-only
    // early return below keeps the old link objects alive — without reuse,
    // refreshed detail/metadata would never reach the rendered links.
    const byLinkKey = linksByKeyRef.current;
    const incomingLinkKeys = new Set<string>();
    for (const edge of edges) {
      const s = edge.source;
      const t = edge.target;
      if (!s || !t) continue;
      degree.set(s, (degree.get(s) ?? 0) + 1);
      degree.set(t, (degree.get(t) ?? 0) + 1);
      const key = str(edge.id, `${s}>${t}:${str(edge.edge_type, "")}`);
      let link: GLink;
      if (incomingLinkKeys.has(key)) {
        // Duplicate key in one payload: fall back to a fresh object so the
        // engine never sees the same link twice.
        link = { ...edge } as GLink;
      } else {
        incomingLinkKeys.add(key);
        const existing = byLinkKey.get(key);
        if (existing) {
          // Refresh payload fields but keep the engine-resolved endpoints.
          const fresh = { ...edge } as Partial<GLink>;
          delete fresh.source;
          delete fresh.target;
          Object.assign(existing, fresh);
          link = existing;
        } else {
          link = { ...edge } as GLink;
          byLinkKey.set(key, link);
        }
      }
      link.__tone = edgeTone(edge);
      links.push(link);
    }
    for (const key of Array.from(byLinkKey.keys())) {
      if (!incomingLinkKeys.has(key)) byLinkKey.delete(key);
    }

    const outNodes: GNode[] = [];
    for (const node of nodes) {
      incomingIds.add(node.id);
      let obj = byId.get(node.id);
      if (obj) {
        Object.assign(obj, node); // refresh payload fields, preserve x/y/vx/vy
      } else {
        obj = { ...node } as GNode;
        byId.set(node.id, obj);
      }
      const rawLabel = str(node.label, node.id);
      const color = graphNodeColor(node);
      const degreeValue = degree.get(node.id) ?? 0;
      obj.__color = color;
      obj.__rim = lighten(color, 0.26);
      obj.__ring = withAlpha(color, 0.48);
      obj.__r = nodeMarkSize(node, degreeValue);
      // Cap the on-canvas label length (the previous renderer truncated to
      // ~160px); truncate by code point so surrogate pairs never split. The
      // full text still shows in the hover tooltip and the inspector panel.
      const codePoints = Array.from(rawLabel);
      obj.__label =
        codePoints.length > 30 ? `${codePoints.slice(0, 29).join("")}…` : rawLabel;
      obj.__degree = degreeValue;
      outNodes.push(obj);
    }
    // Drop nodes that left the result so the cache can't grow unbounded or revive stale positions.
    for (const id of Array.from(byId.keys())) {
      if (!incomingIds.has(id)) byId.delete(id);
    }

    const idKey = outNodes
      .map((n) => String(n.id))
      .sort()
      .join("|");
    const edgeKey = links
      .map((l) => `${idOf(l.source)}>${idOf(l.target)}:${str(l.edge_type, "")}`)
      .sort()
      .join("|");

    // Same node + edge set as last time (a field-only refresh): return the SAME
    // graphData reference so the engine does not re-heat — no jitter, the camera
    // holds, and the existing adjacency/highlight Sets stay valid (they key links
    // by identity). The node objects were already refreshed in place above.
    if (
      lastGraphDataRef.current &&
      idKey === prevIdsKeyRef.current &&
      edgeKey === prevEdgeKeyRef.current
    ) {
      return lastGraphDataRef.current;
    }

    // Structural change (mode/focus switch, added/removed memory or relation):
    // rebuild adjacency, allow exactly one re-fit, and publish a fresh reference
    // (which intentionally lets the engine re-heat and re-settle).
    const neighbors = new Map<string, Set<string>>();
    const linksByNode = new Map<string, Set<GLink>>();
    const bucket = <V,>(map: Map<string, Set<V>>, key: string): Set<V> => {
      let set = map.get(key);
      if (!set) {
        set = new Set<V>();
        map.set(key, set);
      }
      return set;
    };
    for (const link of links) {
      const s = idOf(link.source);
      const t = idOf(link.target);
      if (!s || !t) continue;
      bucket(neighbors, s).add(t);
      bucket(neighbors, t).add(s);
      bucket(linksByNode, s).add(link);
      bucket(linksByNode, t).add(link);
    }
    neighborsRef.current = neighbors;
    linksByNodeRef.current = linksByNode;

    prevIdsKeyRef.current = idKey;
    prevEdgeKeyRef.current = edgeKey;
    didFitRef.current = false;

    const next = { nodes: outNodes, links };
    lastGraphDataRef.current = next;
    return next;
  }, [nodes, edges]);

  // Callback ref: (re)attach the ResizeObserver every time the canvas wrapper
  // mounts. It unmounts on the empty state, so a one-time effect would observe a
  // detached node forever and never re-observe the remounted div. The canvas needs
  // explicit pixel width/height — it does not auto-fit its parent.
  const setWrap = useCallback((el: HTMLDivElement | null) => {
    observerRef.current?.disconnect();
    if (!el) {
      observerRef.current = null;
      setSize({ width: 0, height: 0 });
      return;
    }
    const observer = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      setSize((prev) =>
        prev.width === width && prev.height === height ? prev : { width, height },
      );
    });
    observer.observe(el);
    observerRef.current = observer;
  }, []);

  // Configure the simulation once the imperative handle exists (re-applied on resize).
  useEffect(() => {
    const fg = fgRef.current;
    if (!fg) return;
    const setForce = fg.d3Force.bind(fg) as (name: string, force?: unknown) => unknown;
    // Repulsion is the spread driver; cap its range so far nodes stay cheap.
    setForce("charge", forceManyBody<GNode>().strength(-122).distanceMax(460).theta(0.9));
    // Keep d3's degree-aware default link strength (1/min(deg)) so hubs don't explode — only set distance.
    const linkForce = fg.d3Force("link") as { distance?: (d: number) => unknown } | undefined;
    linkForce?.distance?.(46);
    // Positioning forces toward origin (viewport centre) give a bounded, airy cloud.
    setForce("center", null);
    setForce("x", forceX<GNode>(0).strength(0.07));
    setForce("y", forceY<GNode>(0).strength(0.07));
    // Extra padding gives hover labels room without forcing dense graphs apart.
    setForce(
      "collide",
      forceCollide<GNode>()
        .radius((n) => (n.__r ?? 6) + 3)
        .strength(0.82),
    );
    // The canvas dimensions changed, so re-fit once after this reheat settles
    // (onEngineStop is otherwise guarded against re-fitting and would leave the
    // graph framed to the old size).
    didFitRef.current = false;
    fg.d3ReheatSimulation();
  }, [size.width, size.height]);

  // Pause the render loop on unmount so it can't leak a rAF after route change.
  useEffect(() => {
    return () => {
      (fgRef.current as { pauseAnimation?: () => void } | undefined)?.pauseAnimation?.();
    };
  }, []);

  const paintNode = useCallback(
    (node: GNode, ctx: CanvasRenderingContext2D, globalScale: number) => {
      const x = node.x ?? 0;
      const y = node.y ?? 0;
      const r = node.__r ?? 6;
      const baseColor = node.__color ?? MEMORY_DEFAULT;
      const selId = selectedIdRef.current;
      const isSel = selId != null && String(node.id) === selId;
      const hovActive = hoverRef.current != null;
      const isHoverTarget = hoverRef.current === String(node.id);
      const inHot = highlightNodesRef.current.has(String(node.id));
      // The active selection stays fully lit even while hovering an unrelated node.
      const isHot = !hovActive || inHot || isSel;

      ctx.save();
      ctx.translate(x, y);
      ctx.globalAlpha = isHot ? 1 : 0.18;

      // nodes flat-fill — at 13% alpha the highlight is invisible anyway.
      traceNodeMark(ctx, node, r);
      ctx.fillStyle = baseColor;
      ctx.fill();
      ctx.lineWidth = 1 / globalScale;
      ctx.strokeStyle = node.__rim ?? baseColor;
      ctx.stroke();

      if (node.pinned) {
        traceNodeMark(ctx, node, r + 2.6 / globalScale);
        ctx.lineWidth = 1 / globalScale;
        ctx.strokeStyle = node.__ring ?? baseColor;
        ctx.stroke();
      }

      if (isSel) {
        traceNodeMark(ctx, node, r + 4 / globalScale);
        ctx.lineWidth = 1.25 / globalScale;
        ctx.strokeStyle = ACCENT;
        ctx.stroke();
      }

      if (isHoverTarget && node.__label) {
        const fontSize = (node.node_type === "entity" ? 10.5 : 9.6) / globalScale;
        const fontWeight = node.node_type === "entity" ? 500 : 400;
        ctx.font = `${fontWeight} ${fontSize}px ${MONO_FONT}`;
        ctx.textAlign = "center";
        ctx.textBaseline = "top";
        const top = r + 4.5 / globalScale;
        ctx.globalAlpha = 1;
        ctx.lineJoin = "round";
        ctx.lineWidth = 3 / globalScale;
        ctx.strokeStyle = "rgba(4, 8, 6, 0.9)";
        ctx.strokeText(node.__label, 0, top);
        ctx.fillStyle = node.node_type === "entity" ? "#dff7ea" : "rgba(211, 224, 217, 0.92)";
        ctx.fillText(node.__label, 0, top);
      }
      ctx.restore();
    },
    [],
  );

  const paintPointerArea = useCallback(
    (node: GNode, color: string, ctx: CanvasRenderingContext2D) => {
      const x = node.x ?? 0;
      const y = node.y ?? 0;
      const r = (node.__r ?? 6) + 2;
      ctx.fillStyle = color;
      ctx.beginPath();
      ctx.arc(x, y, r, 0, TWO_PI);
      ctx.fill();
    },
    [],
  );

  const linkColorFn = useCallback((link: GLink) => {
    const tone = link.__tone ?? "explicit";
    if (highlightLinksRef.current.has(link)) return EDGE_HOT[tone];
    if (hoverRef.current != null) return EDGE_DIMMED;
    return LINK_BASE[tone];
  }, []);

  const linkWidthFn = useCallback((link: GLink) => {
    const base = link.__tone === "relation" ? 1.05 : 0.75;
    return highlightLinksRef.current.has(link) ? 1.6 : base;
  }, []);

  const linkCurvatureFn = useCallback(
    () => 0,
    [],
  );

  const handleNodeHover = useCallback((node: GNode | null) => {
    const hot = highlightNodesRef.current;
    const hotLinks = highlightLinksRef.current;
    hot.clear();
    hotLinks.clear();
    if (node && node.id != null) {
      const id = String(node.id);
      hot.add(id);
      neighborsRef.current.get(id)?.forEach((n) => hot.add(n));
      linksByNodeRef.current.get(id)?.forEach((l) => hotLinks.add(l));
      hoverRef.current = id;
    } else {
      hoverRef.current = null;
    }
  }, []);

  const handleNodeClick = useCallback((node: GNode) => {
    setSelectedNode(node as unknown as MemoryGraphNode);
    setSelectedEdge(null);
  }, []);

  const handleLinkClick = useCallback((link: GLink) => {
    setSelectedEdge(link as unknown as MemoryGraphEdge);
    setSelectedNode(null);
  }, []);

  // Release the drag pin so the node eases back into a physics-natural spot.
  const handleNodeDragEnd = useCallback((node: GNode) => {
    node.fx = undefined;
    node.fy = undefined;
  }, []);

  const handleEngineStop = useCallback(() => {
    if (!didFitRef.current) {
      didFitRef.current = true;
      fgRef.current?.zoomToFit(400, 60);
    }
  }, []);

  const relationId = str(asRecord(selectedEdge?.metadata).relation_id, "");
  const evidence = selectedEdge ? inspectorEvidence(selectedEdge) : [];

  return (
    <Box className="list-shell">
      <Stack spacing={1.25}>
        <Stack
          direction="row"
          spacing={1}
          useFlexGap
          sx={{ alignItems: "center", flexWrap: "wrap" }}
        >
          <ToggleButtonGroup
            exclusive
            size="small"
            value={mode}
            onChange={(_event, next) => {
              if (next === "map" || next === "focus") setMode(next);
            }}
          >
            <ToggleButton value="map">All</ToggleButton>
            <ToggleButton value="focus">Selected</ToggleButton>
          </ToggleButtonGroup>
          {mode === "focus" ? (
            <Box
              component="input"
              value={focusId}
              onChange={(event) => setFocusId(event.currentTarget.value)}
              placeholder="Paste memory id"
              sx={{
                height: 34,
                borderRadius: 1,
                border: "1px solid var(--surface-border)",
                background: "rgba(7, 11, 9, 0.6)",
                color: "text.primary",
                px: 1,
                font: "inherit",
                fontSize: 13,
                minWidth: 240,
              }}
            />
          ) : null}
          <Tooltip title="Show embedding-nearby memories">
            <FormControlLabel
              control={
                <Switch
                  size="small"
                  checked={includeSemantic}
                  onChange={(event) => setIncludeSemantic(event.currentTarget.checked)}
                />
              }
              label="Nearby"
            />
          </Tooltip>
          <Tooltip title="Refresh graph">
            <IconButton size="small" onClick={() => graphQ.refetch()} disabled={graphQ.isFetching}>
              <RefreshCw size={17} />
            </IconButton>
          </Tooltip>
          <Chip size="small" variant="outlined" label={summary} />
        </Stack>

        {graphQ.error ? <Alert severity="error">{errMessage(graphQ.error)}</Alert> : null}

        <Stack direction={{ xs: "column", lg: "row" }} spacing={1.25}>
          <Box
            className="memgraph-canvas"
            sx={{
              minHeight: { xs: 520, lg: 640 },
              height: { xs: 520, lg: 640 },
              flex: 1,
            }}
          >
            {nodes.length === 0 && !graphQ.isFetching ? (
              <Stack
                sx={{
                  height: "100%",
                  alignItems: "center",
                  justifyContent: "center",
                  color: "text.secondary",
                }}
                spacing={1}
              >
                <Search size={22} />
                <Typography variant="body2">No memories to show yet.</Typography>
                <Typography
                  variant="caption"
                  sx={{ fontFamily: "var(--font-mono)", fontSize: 10.5, color: "text.disabled" }}
                >
                  Captured memories and entities appear here as a graph.
                </Typography>
              </Stack>
            ) : (
              <div ref={setWrap} style={{ position: "absolute", inset: 0 }}>
                {legendEntries.length > 0 ? (
                  <Box className="memgraph-legend">
                  {legendEntries.map((entry) => (
                    <Box key={entry.label} className="memgraph-legend-item">
                      <Box
                        className="memgraph-legend-dot"
                        sx={{ background: entry.color }}
                      />
                      <span>{entry.label}</span>
                      <span className="memgraph-legend-count">{entry.count}</span>
                    </Box>
                  ))}
                  {(payload.semantic_edge_count ?? 0) > 0 ? (
                    <Box className="memgraph-legend-item">
                      <Box className="memgraph-legend-dash" />
                      <span>Semantic</span>
                    </Box>
                  ) : null}
                  </Box>
                ) : null}
                {size.width > 0 && size.height > 0 ? (
                  <ForceGraph2D<MemoryNodeExtra, MemoryEdgeExtra>
                    ref={fgRef}
                    width={size.width}
                    height={size.height}
                    graphData={graphData}
                    backgroundColor="rgba(0,0,0,0)"
                    nodeCanvasObject={paintNode}
                    nodeCanvasObjectMode={() => "replace"}
                    nodePointerAreaPaint={paintPointerArea}
                    nodeLabel={(node: GNode) => graphTooltip({ data: node })}
                    linkLabel={(link: GLink) => graphTooltip({ dataType: "edge", data: link })}
                    linkColor={linkColorFn}
                    linkWidth={linkWidthFn}
                    linkCurvature={linkCurvatureFn}
                    onNodeHover={handleNodeHover}
                    onNodeClick={handleNodeClick}
                    onLinkClick={handleLinkClick}
                    onNodeDragEnd={handleNodeDragEnd}
                    onEngineStop={handleEngineStop}
                    cooldownTicks={120}
                    d3VelocityDecay={0.4}
                    d3AlphaDecay={0.0182}
                    minZoom={0.1}
                    maxZoom={8}
                    autoPauseRedraw={false}
                  />
                ) : null}
              </div>
            )}
          </Box>

          <Box
            sx={{
              width: { xs: "100%", lg: 340 },
              border: "1px solid rgba(120, 242, 176, 0.14)",
              borderRadius: "var(--surface-radius-lg)",
              p: 1.5,
              alignSelf: "stretch",
              background:
                "linear-gradient(180deg, rgba(120, 242, 176, 0.04), transparent 42%), rgba(7, 11, 9, 0.6)",
            }}
          >
            <Typography
              sx={{
                fontFamily: "var(--font-mono)",
                fontSize: 10,
                letterSpacing: "0.14em",
                textTransform: "uppercase",
                color: "rgba(120, 242, 176, 0.7)",
                mb: 1,
              }}
            >
              Inspector
            </Typography>
            {selectedEdge ? (
              <Stack spacing={1}>
                <Stack direction="row" spacing={0.75} sx={{ alignItems: "center" }}>
                  <Box
                    sx={{
                      width: 14,
                      height: 2,
                      borderRadius: 1,
                      flex: "none",
                      background: EDGE_HOT[edgeTone(selectedEdge) ?? "explicit"],
                    }}
                  />
                  <Typography variant="subtitle2" sx={{ fontWeight: 700, flex: 1 }}>
                    {memoryGraphEdgeLabel(selectedEdge)}
                  </Typography>
                  <IconButton size="small" onClick={() => setSelectedEdge(null)}>
                    <X size={15} />
                  </IconButton>
                </Stack>
                <Chip
                  size="small"
                  variant="outlined"
                  label={humanizeMachineLabel(str(selectedEdge.edge_type, "link"))}
                  sx={{ alignSelf: "flex-start" }}
                />
                <Typography variant="body2" sx={{ color: "text.secondary" }}>
                  {str(selectedEdge.detail, "No detail recorded.")}
                </Typography>
                {relationId ? (
                  <Stack direction="row" spacing={0.75}>
                    <Button
                      size="small"
                      variant="contained"
                      startIcon={<Check size={15} />}
                      disabled={relationStatusMutation.isPending}
                      onClick={() =>
                        relationStatusMutation.mutate({
                          id: relationId,
                          action: "confirm",
                        })
                      }
                    >
                      Confirm
                    </Button>
                    <Button
                      size="small"
                      color="warning"
                      variant="outlined"
                      disabled={relationStatusMutation.isPending}
                      onClick={() =>
                        relationStatusMutation.mutate({
                          id: relationId,
                          action: "reject",
                        })
                      }
                    >
                      Reject
                    </Button>
                  </Stack>
                ) : null}
                {evidence.length > 0 ? (
                  <>
                    <Divider />
                    <Stack spacing={0.75}>
                      {evidence.map((item, index) => (
                        <Box key={`${str(item.id, "evidence")}-${index}`}>
                          <Typography variant="caption" sx={{ color: "text.secondary" }}>
                            {humanizeMachineLabel(str(item.evidence_kind, "evidence"))}
                          </Typography>
                          <Typography variant="body2">
                            {str(item.excerpt, str(item.evidence_ref, ""))}
                          </Typography>
                        </Box>
                      ))}
                    </Stack>
                  </>
                ) : null}
              </Stack>
            ) : selectedNode ? (
              <Stack spacing={1}>
                <Stack direction="row" spacing={0.75} sx={{ alignItems: "center" }}>
                  <Box
                    sx={{
                      width: 9,
                      height: 9,
                      borderRadius: str(selectedNode.node_type, "memory") === "entity" ? "2px" : "1px",
                      flex: "none",
                      background: graphNodeColor(selectedNode),
                      transform: str(selectedNode.node_type, "memory") === "memory" ? "rotate(45deg)" : "none",
                    }}
                  />
                  <Typography variant="subtitle2" sx={{ fontWeight: 700, flex: 1 }}>
                    {str(selectedNode.label, selectedNode.id)}
                  </Typography>
                  <IconButton size="small" onClick={() => setSelectedNode(null)}>
                    <X size={15} />
                  </IconButton>
                </Stack>
                <Stack direction="row" spacing={0.75} useFlexGap sx={{ flexWrap: "wrap" }}>
                  <Chip
                    size="small"
                    variant="outlined"
                    label={humanizeMachineLabel(str(selectedNode.node_type, "memory"))}
                  />
                  {selectedNode.category ? (
                    <Chip
                      size="small"
                      variant="outlined"
                      label={humanizeMachineLabel(selectedNode.category)}
                    />
                  ) : null}
                  {selectedNode.status ? (
                    <Chip
                      size="small"
                      variant="outlined"
                      label={humanizeMachineLabel(selectedNode.status)}
                    />
                  ) : null}
                </Stack>
                <Typography variant="body2" sx={{ color: "text.secondary" }}>
                  {str(selectedNode.detail, "No detail recorded.")}
                </Typography>
                {Number.isFinite(selectedNode.confidence ?? NaN) ? (
                  <Stack spacing={0.5}>
                    <Stack direction="row" sx={{ justifyContent: "space-between" }}>
                      <Typography
                        variant="caption"
                        sx={{ fontFamily: "var(--font-mono)", fontSize: 10.5, color: "text.secondary" }}
                      >
                        confidence
                      </Typography>
                      <Typography
                        variant="caption"
                        sx={{ fontFamily: "var(--font-mono)", fontSize: 10.5 }}
                      >
                        {Math.round(num(selectedNode.confidence, 0) * 100)}%
                        {num(selectedNode.support_count, 0) > 0
                          ? ` · ×${num(selectedNode.support_count, 0)}`
                          : ""}
                      </Typography>
                    </Stack>
                    <Box sx={{ height: 3, borderRadius: 2, background: "rgba(255, 255, 255, 0.07)" }}>
                      <Box
                        sx={{
                          height: "100%",
                          borderRadius: 2,
                          width: `${Math.round(clamp(num(selectedNode.confidence, 0), 0, 1) * 100)}%`,
                          // Same confidence bands as the Current Memory list.
                          background:
                            num(selectedNode.confidence, 0) >= 0.85
                              ? ACCENT
                              : num(selectedNode.confidence, 0) >= 0.6
                                ? "#ffbe63"
                                : "#ff9b9b",
                        }}
                      />
                    </Box>
                  </Stack>
                ) : null}
                <Typography
                  variant="caption"
                  sx={{
                    fontFamily: "var(--font-mono)",
                    fontSize: 10,
                    color: "text.disabled",
                    overflowWrap: "anywhere",
                  }}
                >
                  {selectedNode.id}
                </Typography>
              </Stack>
            ) : (
              <Stack spacing={1.25} sx={{ color: "text.secondary" }}>
                <Typography variant="body2">Click a node or link to inspect it.</Typography>
                <Divider sx={{ borderColor: "rgba(255, 255, 255, 0.06)" }} />
                <Stack spacing={0.6}>
                  {[
                    ["hover", "spotlight neighbors"],
                    ["click", "inspect details"],
                    ["drag", "reposition node"],
                    ["scroll", "zoom canvas"],
                  ].map(([key, hint]) => (
                    <Stack key={key} direction="row" spacing={1}>
                      <span className="memgraph-hint-key">{key}</span>
                      <span className="memgraph-hint-value">{hint}</span>
                    </Stack>
                  ))}
                </Stack>
              </Stack>
            )}
          </Box>
        </Stack>
      </Stack>
    </Box>
  );
}
