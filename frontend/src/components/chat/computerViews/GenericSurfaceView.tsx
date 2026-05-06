import Box from "@mui/material/Box";
import Typography from "@mui/material/Typography";
import AutoAwesomeRoundedIcon from "@mui/icons-material/AutoAwesomeRounded";

import type { ChatStepCard, SurfaceArtifact, SurfacePayload } from "../types";
import { firstSurfaceText, surfaceDisplayTitle, surfaceFromCard, surfacePayloads } from "../surface";

export interface GenericSurfaceViewProps {
  card: ChatStepCard;
}

function bodyFromItem(item: SurfacePayload | SurfaceArtifact): string {
  if (item.text) return item.text;
  if (item.preview) return item.preview;
  if (item.json != null) {
    try {
      return JSON.stringify(item.json, null, 2);
    } catch {
      return "";
    }
  }
  if (item.uri) return item.uri;
  if (item.path) return item.path;
  return "";
}

function normalizedBodyKey(body: string): string {
  return body.trim().replace(/\s+/g, " ");
}

function labelForItem(item: SurfacePayload | SurfaceArtifact): string {
  return ("label" in item && item.label) || item.role;
}

function uniqueSurfaceRows(items: Array<SurfacePayload | SurfaceArtifact>) {
  const rows: Array<{ key: string; label: string; contentType: string; body: string }> = [];
  const rowByBody = new Map<string, number>();

  for (const item of items) {
    const body = bodyFromItem(item);
    const bodyKey = normalizedBodyKey(body);
    if (!bodyKey) continue;

    const label = labelForItem(item);
    const existingIndex = rowByBody.get(bodyKey);
    if (existingIndex != null) {
      const existing = rows[existingIndex];
      const labels = existing.label
        .split(" / ")
        .map((value) => value.trim())
        .filter(Boolean);
      if (label && !labels.includes(label)) {
        existing.label = [...labels, label].join(" / ");
      }
      continue;
    }

    rowByBody.set(bodyKey, rows.length);
    rows.push({
      key: `${item.role}-${rows.length}`,
      label,
      contentType: item.contentType,
      body,
    });
  }

  return rows;
}

export function GenericSurfaceView({ card }: GenericSurfaceViewProps) {
  const surface = surfaceFromCard(card);
  const title = surfaceDisplayTitle(card);
  const items = surfacePayloads(card);
  const fallback =
    firstSurfaceText(card) ||
    card.rawDetailFull ||
    card.detailFull ||
    card.detail ||
    card.summary ||
    "";

  return (
    <Box className="cview cview-generic">
      <Box className="cview-generic-head">
        <AutoAwesomeRoundedIcon fontSize="small" className="cview-generic-icon" />
        <Typography variant="subtitle2" className="cview-generic-title">
          {title}
        </Typography>
        {surface?.renderer.id ? (
          <span className="cview-generic-renderer">{surface.renderer.id}</span>
        ) : null}
      </Box>
      {items.length > 0 ? (
        <StacklessSurfaceItems items={items} />
      ) : fallback ? (
        <details className="cview-generic-item cview-generic-item--single">
          <summary className="cview-generic-item-head">
            <span>Raw payload</span>
            <span className="cview-generic-item-meta">
              <span className="cview-generic-item-bytes">{fallback.length.toLocaleString()} chars</span>
              <span className="cview-generic-chev" aria-hidden="true">▸</span>
            </span>
          </summary>
          <pre className="cview-generic-body">{fallback}</pre>
        </details>
      ) : (
        <Typography variant="body2" className="cview-generic-empty">
          No structured artifact was captured for this step.
        </Typography>
      )}
    </Box>
  );
}

function StacklessSurfaceItems({
  items,
}: {
  items: Array<SurfacePayload | SurfaceArtifact>;
}) {
  const rows = uniqueSurfaceRows(items);
  if (rows.length === 0) {
    return (
      <Typography variant="body2" className="cview-generic-empty">
        No structured artifact was captured for this step.
      </Typography>
    );
  }

  return (
    <div className="cview-generic-items">
      {rows.map((row) => {
        const charCount = row.body.length;
        return (
          <details key={row.key} className="cview-generic-item">
            <summary className="cview-generic-item-head">
              <span className="cview-generic-item-label">{row.label}</span>
              <span className="cview-generic-item-meta">
                <span className="cview-generic-item-type">{row.contentType}</span>
                <span className="cview-generic-item-bytes">{charCount.toLocaleString()} chars</span>
                <span className="cview-generic-chev" aria-hidden="true">▸</span>
              </span>
            </summary>
            <pre className="cview-generic-body">{row.body}</pre>
          </details>
        );
      })}
    </div>
  );
}

export default GenericSurfaceView;
