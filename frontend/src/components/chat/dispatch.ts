// Computer-pane dispatch is intentionally structural. It uses the open
// SurfaceDescriptor emitted by the runtime and does not route by labels,
// summaries, prompt text, display phrasing, or keyword combinations.

import type { ChatStepCard, ChipStatus, ComputerViewKind } from "./types";
import {
  AGENTARK_RENDERERS,
  firstSurfaceCommand,
  firstSurfacePath,
  firstSurfaceText,
  firstSurfaceUri,
  isRegisteredWorkspaceSurface,
  rendererIdForCard,
  surfaceFromCard,
  surfaceGroupKey,
  surfaceStatus,
} from "./surface";

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
}

function str(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function tryParseRecord(raw: string | undefined): Record<string, unknown> | null {
  const body = (raw || "").trim();
  if (!body || body[0] !== "{") return null;
  try {
    const parsed = JSON.parse(body) as unknown;
    return isRecord(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function structuredRecord(card: ChatStepCard): Record<string, unknown> | null {
  return (
    tryParseRecord(card.payloadView?.body) ||
    tryParseRecord(card.rawDetailFull) ||
    tryParseRecord(card.detailFull)
  );
}

export function pickComputerView(card: ChatStepCard): ComputerViewKind {
  return rendererIdForCard(card);
}

export function extractUrl(card: ChatStepCard): string {
  const fromSurface = firstSurfaceUri(card);
  if (fromSurface) return fromSurface;
  const record = structuredRecord(card);
  return str(record?.url) || str(record?.uri) || str(record?.href);
}

export function extractFilePath(card: ChatStepCard): string {
  const fromSurface = firstSurfacePath(card);
  if (fromSurface) return fromSurface;
  const record = structuredRecord(card);
  const direct = str(record?.file) || str(record?.path);
  if (direct) return direct;
  const files = record?.files;
  if (isRecord(files)) {
    return Object.keys(files).find(Boolean) || "";
  }
  if (Array.isArray(files)) {
    for (const entry of files) {
      if (!isRecord(entry)) continue;
      const path = str(entry.path) || str(entry.file) || str(entry.name);
      if (path) return path;
    }
  }
  return "";
}

export function extractCommand(card: ChatStepCard): string {
  const fromSurface = firstSurfaceCommand(card);
  if (fromSurface) return fromSurface.trim();
  const record = structuredRecord(card);
  if (!record) return "";
  const args = isRecord(record.args) ? record.args : isRecord(record.arguments) ? record.arguments : {};
  return (
    str(record.command) ||
    str(record.cmd) ||
    str(args.command) ||
    str(args.cmd)
  ).trim();
}

export function extractSurfaceBody(card: ChatStepCard): string {
  const output = firstSurfaceText(card, ["stdout", "stderr", "transcript", "output", "result", "content"]);
  if (output) return output;
  return firstSurfaceText(card);
}

export function chipStatusFromCard(
  card: ChatStepCard,
  isLastLive: boolean,
  runIsLive: boolean = true,
): ChipStatus {
  const status = surfaceStatus(card, isLastLive && runIsLive);
  if (status === "error") return "issue";
  if (status === "done") return "done";
  if (status === "running" || status === "waiting" || status === "pending") {
    return isLastLive && runIsLive ? "running" : "idle";
  }
  return "idle";
}

export function collapseChipCards(cards: ChatStepCard[]): ChatStepCard[] {
  const out: ChatStepCard[] = [];
  const indexByKey = new Map<string, number>();
  for (const card of cards) {
    const key = surfaceGroupKey(card);
    const existingIndex = indexByKey.get(key);
    if (existingIndex == null) {
      indexByKey.set(key, out.length);
      out.push(card);
    } else {
      out[existingIndex] = card;
    }
  }
  return out;
}

export function prepareChipCards(cards: ChatStepCard[]): ChatStepCard[] {
  if (!cards || cards.length === 0) return [];
  return collapseChipCards(
    cards.filter((card) => !card.isHeartbeat && isRegisteredWorkspaceSurface(card)),
  ).filter((card) => {
    const renderer = surfaceFromCard(card)?.renderer.id || AGENTARK_RENDERERS.GENERIC;
    return renderer !== AGENTARK_RENDERERS.WORKING;
  });
}
