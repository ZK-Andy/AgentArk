import type { ChatStepCard } from "../types";
import {
  firstSurfaceCommand,
  firstSurfaceText,
  surfaceDisplayTitle,
  surfaceFromCard,
} from "../surface";

export interface ReadableToolPresentation {
  title: string;
  query: string;
  summary: string;
  rows: string[];
  body: string;
  toolName: string;
  isStructured: boolean;
}

type JsonRecord = Record<string, unknown>;

const MAX_ROWS = 10;

function str(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function asRecord(value: unknown): JsonRecord {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as JsonRecord)
    : {};
}

function tryParseJson(raw: unknown): unknown | null {
  const text = str(raw).trim();
  if (!text) return null;
  const first = text[0];
  if (first !== "{" && first !== "[") return null;
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return null;
  }
}

function firstParsedValue(values: unknown[]): unknown | null {
  for (const value of values) {
    const parsed = tryParseJson(value);
    if (parsed != null) return parsed;
  }
  return null;
}

function parsedCardValue(card: ChatStepCard): unknown | null {
  return firstParsedValue([
    card.payloadView?.body,
    card.rawDetailFull,
    card.detailFull,
  ]);
}

function nestedStructuredValue(record: JsonRecord): unknown | null {
  for (const key of ["raw_content", "result", "response", "payload", "data"]) {
    const value = record[key];
    if (typeof value === "string") {
      const parsed = tryParseJson(value);
      if (parsed != null) return parsed;
    } else if (value && typeof value === "object") {
      return value;
    }
  }
  return null;
}

function presentationValue(value: unknown): unknown {
  const record = asRecord(value);
  if (Object.keys(record).length === 0) return value;

  const nested = nestedStructuredValue(record);
  if (!nested) return value;

  const nestedRecord = asRecord(nested);
  if (
    Object.keys(nestedRecord).length > 0 &&
    (nestedRecord.results != null || nestedRecord.items != null)
  ) {
    return nested;
  }

  const ownUsefulShape =
    record.results != null ||
    record.items != null ||
    record.matches != null ||
    record.files != null;
  return ownUsefulShape ? value : nested;
}

function formatIdentifier(value: string): string {
  const normalized = (value || "")
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/[_-]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
  if (!normalized) return "";
  return normalized.replace(/\b\w/g, (ch) => ch.toUpperCase());
}

function formatToolName(value: string): string {
  return formatIdentifier(value) || "Tool";
}

function compactText(value: string, limit = 220): string {
  const text = value.replace(/\s+/g, " ").trim();
  if (text.length <= limit) return text;
  return `${text.slice(0, Math.max(0, limit - 3)).trimEnd()}...`;
}

function cleanMachinePrefix(value: string): string {
  const text = value.replace(/\s+/g, " ").trim();
  const match = text.match(/^([A-Za-z][\w.-]{1,80})\s*:\s+(.+)$/);
  if (!match) return text;
  const prefix = match[1] || "";
  const rest = match[2] || "";
  if (!rest.trim()) return text;
  if (prefix.includes("_") || prefix.includes(".") || !prefix.includes(" ")) {
    return rest.trim();
  }
  return text;
}

function looksLikeMetadataValue(value: string): boolean {
  const text = value.trim();
  return (
    /^[0-9a-f]{8,}(-[0-9a-f]{4,}){2,}$/i.test(text) ||
    /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/.test(text) ||
    /^https?:\/\/\S+$/i.test(text)
  );
}

function fieldScore(key: string, value: string): number {
  const normalizedKey = key.toLowerCase();
  const text = cleanMachinePrefix(value);
  if (!text || looksLikeMetadataValue(text) || tryParseJson(text) != null)
    return -100;

  let score = Math.min(text.length, 220);
  if (/\s/.test(text)) score += 40;
  if (/[.!?]$/.test(text)) score += 12;
  if (
    /content|summary|message|detail|description|title|value|text/.test(
      normalizedKey,
    )
  ) {
    score += 80;
  }
  if (
    /id|uuid|timestamp|updated|created|confidence|score|rate|count|status/.test(
      normalizedKey,
    )
  ) {
    score -= 80;
  }
  return score;
}

function bestRecordText(record: JsonRecord): string {
  const candidates = Object.entries(record)
    .filter(([, value]) => typeof value === "string")
    .map(([key, value]) => ({
      key,
      value: compactText(cleanMachinePrefix(str(value))),
      score: fieldScore(key, str(value)),
    }))
    .filter((entry) => entry.value && entry.score > -50)
    .sort((left, right) => right.score - left.score);

  if (candidates[0]) return candidates[0].value;

  const keys = Object.keys(record)
    .filter((key) => record[key] != null)
    .slice(0, 4)
    .map(formatIdentifier)
    .filter(Boolean);
  return keys.length > 0 ? `Structured item: ${keys.join(", ")}.` : "";
}

function scalarText(value: unknown): string {
  if (typeof value === "string") return compactText(cleanMachinePrefix(value));
  if (typeof value === "number" || typeof value === "boolean")
    return String(value);
  return "";
}

function rowsFromArray(items: unknown[], bucketLabel = ""): string[] {
  return items
    .map((item) => {
      const record = asRecord(item);
      const text =
        Object.keys(record).length > 0
          ? bestRecordText(record)
          : scalarText(item);
      if (!text) return "";
      return bucketLabel ? `${bucketLabel}: ${text}` : text;
    })
    .filter(Boolean);
}

function visibleArrayBuckets(record: JsonRecord): Array<{
  label: string;
  count: number;
  rows: string[];
}> {
  const buckets: Array<{ label: string; count: number; rows: string[] }> = [];
  for (const [key, value] of Object.entries(record)) {
    if (!Array.isArray(value)) continue;
    const label = formatIdentifier(key);
    buckets.push({
      label,
      count: value.length,
      rows: rowsFromArray(value, label),
    });
  }
  return buckets;
}

function resultBuckets(value: unknown): Array<{
  label: string;
  count: number;
  rows: string[];
}> {
  if (Array.isArray(value)) {
    return [
      {
        label: "Results",
        count: value.length,
        rows: rowsFromArray(value),
      },
    ];
  }
  const record = asRecord(value);
  if (Object.keys(record).length === 0) return [];
  const nestedResults = record.results;
  if (Array.isArray(nestedResults)) {
    return resultBuckets(nestedResults);
  }
  const nestedResultRecord = asRecord(nestedResults);
  if (Object.keys(nestedResultRecord).length > 0) {
    return visibleArrayBuckets(nestedResultRecord);
  }
  return visibleArrayBuckets(record);
}

function summarizeBuckets(
  buckets: Array<{ label: string; count: number }>,
  query: string,
): string {
  const total = buckets.reduce((sum, bucket) => sum + bucket.count, 0);
  const queryPart = query ? ` for "${compactText(query, 72)}"` : "";
  if (total === 0) {
    return queryPart ? `No results found${queryPart}.` : "No items returned.";
  }

  const nonEmpty = buckets.filter((bucket) => bucket.count > 0).slice(0, 3);
  const bucketPart =
    nonEmpty.length > 0
      ? ` in ${nonEmpty
          .map((bucket) => {
            let label = bucket.label.toLowerCase();
            if (bucket.count === 1 && label.endsWith("s")) {
              label = label.slice(0, -1);
            }
            return `${bucket.count} ${label}`;
          })
          .join(", ")}`
      : "";
  const noun = queryPart ? "result" : "item";
  const verb = queryPart ? "Found" : "Collected";
  return `${verb} ${total} ${noun}${total === 1 ? "" : "s"}${bucketPart}${queryPart}.`;
}

function queryFromValue(value: unknown): string {
  const record = asRecord(value);
  return compactText(str(record.query, str(record.search, "")), 110);
}

function contentTextFromRecord(record: JsonRecord): string {
  const content = scalarText(record.content);
  if (content) return content;
  const message = scalarText(record.message);
  if (message) return message;
  const summary = scalarText(record.summary);
  if (summary) return summary;
  return "";
}

function fallbackRowsFromRecord(record: JsonRecord): string[] {
  const rows: string[] = [];
  const best = bestRecordText(record);
  if (best) rows.push(best);
  for (const [, value] of Object.entries(record)) {
    if (rows.length >= MAX_ROWS) break;
    if (!Array.isArray(value)) continue;
    rows.push(...rowsFromArray(value).slice(0, MAX_ROWS - rows.length));
  }
  return rows;
}

export function buildReadableToolPresentation(
  card: ChatStepCard,
): ReadableToolPresentation {
  const surface = surfaceFromCard(card);
  if (surface) {
    const command = firstSurfaceCommand(card);
    const body = firstSurfaceText(card) || surface.error?.message || "";
    const title = surfaceDisplayTitle(card);
    const query =
      command ||
      surface.input
        ?.map((item) => item.preview || item.text || "")
        .find((value) => value.trim()) ||
      "";
    const rows = [...(surface.output || []), ...(surface.artifacts || [])]
      .map((item) => item.preview || item.text || scalarText(item.json))
      .filter(Boolean)
      .slice(0, MAX_ROWS);
    const summary =
      compactText(surface.error?.message || surface.output?.[0]?.preview || body, 260) ||
      `${title} ${surface.status}.`;
    return {
      title,
      query,
      summary,
      rows: rows.length > 0 ? rows : summary ? [summary] : [],
      body: body || rows.join("\n"),
      toolName: surface.tool?.id || "",
      isStructured: true,
    };
  }

  const parsed = parsedCardValue(card);
  const hasStructuredPayloadView = card.payloadView?.kind === "json";
  const structured = parsed != null || hasStructuredPayloadView;
  const value = structured ? presentationValue(parsed) : null;
  const record = asRecord(value);
  const outerRecord = asRecord(parsed);
  const toolName =
    str(
      record.tool_name,
      str(
        record.name,
        str(outerRecord.tool_name, str(outerRecord.name, card.stepType)),
      ),
    ).trim();
  const title = formatToolName(
    toolName || card.label || card.rawTitle || "Tool",
  );
  const query = queryFromValue(value) || queryFromValue(outerRecord);

  if (structured) {
    if (parsed == null) {
      const summary =
        compactText(card.payloadView?.preview || "", 180) ||
        "Received structured tool output.";
      return {
        title,
        query: "",
        summary,
        rows: [summary],
        body: summary,
        toolName,
        isStructured: true,
      };
    }

    const buckets = resultBuckets(value);
    const rows = buckets.flatMap((bucket) => bucket.rows).slice(0, MAX_ROWS);
    if (buckets.length > 0) {
      const summary = summarizeBuckets(buckets, query);
      return {
        title,
        query,
        summary,
        rows: rows.length > 0 ? rows : [summary],
        body: [summary, ...rows].filter(Boolean).join("\n"),
        toolName,
        isStructured: true,
      };
    }

    const content =
      contentTextFromRecord(record) || contentTextFromRecord(outerRecord);
    const fallbackRows = fallbackRowsFromRecord(record).slice(0, MAX_ROWS);
    const summary =
      content ||
      (fallbackRows.length > 0
        ? `Received ${fallbackRows.length} structured item${fallbackRows.length === 1 ? "" : "s"}.`
        : "Received structured tool output.");
    const rowsOut = fallbackRows.length > 0 ? fallbackRows : [summary];
    return {
      title,
      query,
      summary,
      rows: rowsOut,
      body: [summary, ...rowsOut.filter((row) => row !== summary)]
        .filter(Boolean)
        .join("\n"),
      toolName,
      isStructured: true,
    };
  }

  const body =
    card.rawDetailFull ||
    card.detailFull ||
    card.detail ||
    card.summary ||
    card.payloadView?.body ||
    "";
  const summary = compactText(card.summary || card.detail || body, 260);
  return {
    title,
    query: compactText(card.detail || card.summary || card.label, 110),
    summary,
    rows: [],
    body,
    toolName,
    isStructured: false,
  };
}
