// App deploy view for the Computer pane.
// Renders the deployed app URL plus a list of files captured during the deploy.

import { useMemo, useState } from "react";
import Box from "@mui/material/Box";
import Typography from "@mui/material/Typography";
import IconButton from "@mui/material/IconButton";
import Tooltip from "@mui/material/Tooltip";
import OpenInNewRounded from "@mui/icons-material/OpenInNewRounded";
import ContentCopyRounded from "@mui/icons-material/ContentCopyRounded";
import InsertDriveFileRounded from "@mui/icons-material/InsertDriveFileRounded";
import ChevronRightRounded from "@mui/icons-material/ChevronRightRounded";

import type { ChatStepCard, ComputerPaneFile } from "../types";
import { firstSurfaceUri, surfacePayloads } from "../surface";

export interface AppDeployViewProps {
  card: ChatStepCard;
  onOpenFile?: (path: string) => void;
  workspaceFiles?: ComputerPaneFile[];
}

interface DeployFile {
  path: string;
  bytes?: number;
}

interface DeployPayload {
  appId?: string;
  url?: string;
  files: DeployFile[];
}

function safeParse(body: string): unknown {
  try {
    return JSON.parse(body);
  } catch {
    return null;
  }
}

function asNumber(value: unknown): number | undefined {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string") {
    const n = Number(value);
    if (Number.isFinite(n)) return n;
  }
  return undefined;
}

function isDeployFilePath(path: string): boolean {
  const normalized = (path || "").trim().replace(/\\/g, "/");
  if (!normalized) return false;
  if (/^https?:\/\//i.test(normalized)) return false;
  if (/^[\d.]+$/.test(normalized)) return false;
  if (normalized.includes("..") || normalized.startsWith("/")) return false;
  const parts = normalized.split("/").filter(Boolean);
  if (parts.length === 0) return false;
  const base = parts[parts.length - 1] || "";
  if (!base || /[<>:"|?*]/.test(base)) return false;
  return parts.length > 1 || /\.[A-Za-z0-9]{1,12}$/.test(base) || base.startsWith(".");
}

function normalizeFiles(raw: unknown): DeployFile[] {
  if (raw && typeof raw === "object" && !Array.isArray(raw)) {
    return Object.entries(raw as Record<string, unknown>)
      .filter(([path]) => isDeployFilePath(path))
      .map(([path, content]) => ({
        path,
        bytes: typeof content === "string" ? new Blob([content]).size : undefined,
      }));
  }
  if (!Array.isArray(raw)) return [];
  const out: DeployFile[] = [];
  for (const entry of raw) {
    if (!entry || typeof entry !== "object") continue;
    const rec = entry as Record<string, unknown>;
    const path =
      (typeof rec.path === "string" && rec.path) ||
      (typeof rec.file === "string" && rec.file) ||
      (typeof rec.name === "string" && rec.name) ||
      "";
    if (!isDeployFilePath(path)) continue;
    const rawContent =
      typeof rec.content === "string"
        ? rec.content
        : typeof rec.text === "string"
          ? rec.text
          : typeof rec.body === "string"
            ? rec.body
            : "";
    const bytes = asNumber(rec.bytes) ?? asNumber(rec.size) ?? (rawContent ? new Blob([rawContent]).size : undefined);
    out.push({ path, bytes });
  }
  return out;
}

function mergeDeployFiles(
  primary: DeployFile[],
  workspaceFiles: ComputerPaneFile[],
): DeployFile[] {
  const merged = new Map<string, DeployFile>();
  for (const file of [...primary, ...workspaceFiles.map((entry) => ({
    path: entry.path,
    bytes: entry.content ? new Blob([entry.content]).size : undefined,
  }))]) {
    const key = file.path.trim();
    if (!isDeployFilePath(key)) continue;
    const existing = merged.get(key);
    merged.set(key, {
      path: key,
      bytes: file.bytes ?? existing?.bytes,
    });
  }
  return Array.from(merged.values());
}

function parsePayload(card: ChatStepCard): DeployPayload {
  const surfaceUrl = firstSurfaceUri(card);
  const surfaceFiles = normalizeFiles(
    surfacePayloads(card)
      .map((item) => {
        if (item.path) {
          return {
            path: item.path,
            content: item.text || (typeof item.json === "string" ? item.json : ""),
          };
        }
        return null;
      })
      .filter(Boolean),
  );
  const body =
    card.payloadView?.body ||
    card.rawDetailFull ||
    card.detailFull ||
    card.detail ||
    card.summary ||
    "";
  const parsed = safeParse(body);
  let appId: string | undefined;
  let url: string | undefined = surfaceUrl || undefined;
  let files: DeployFile[] = surfaceFiles;
  if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
    const rec = parsed as Record<string, unknown>;
    if (typeof rec.app_id === "string") appId = rec.app_id;
    if (typeof rec.url === "string") url = rec.url;
    else if (typeof rec.app_url === "string") url = rec.app_url;
    else if (typeof rec.dashboard_url === "string") url = rec.dashboard_url;
    files = normalizeFiles(rec.files);
    if (files.length === 0) files = normalizeFiles(rec.sources);
    if (files.length === 0 && Array.isArray(rec.file_names)) {
      files = (rec.file_names as unknown[])
        .filter((value): value is string => typeof value === "string")
        .filter(isDeployFilePath)
        .map((path) => ({ path }));
    }
  }
  return { appId, url, files };
}

function formatKb(bytes?: number): string | null {
  if (bytes === undefined || bytes < 0) return null;
  const kb = bytes / 1024;
  if (kb < 0.1) return `${bytes} B`;
  return `${kb.toFixed(kb < 10 ? 1 : 0)} KB`;
}

export function AppDeployView({
  card,
  onOpenFile,
  workspaceFiles = [],
}: AppDeployViewProps) {
  const payload = useMemo(() => parsePayload(card), [card]);
  const files = useMemo(
    () => mergeDeployFiles(payload.files, workspaceFiles),
    [payload.files, workspaceFiles],
  );
  const [copied, setCopied] = useState(false);

  const title = card.label || payload.appId || "App deploy";
  const handleCopy = () => {
    if (!payload.url) return;
    void navigator.clipboard.writeText(payload.url).then(() => {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    });
  };

  return (
    <Box className="cview cview-deploy">
      <Box className="cview-deploy-head">
        <Box className="cview-deploy-title">
          <Typography component="span" variant="subtitle2">{title}</Typography>
          <Box component="span" className="cview-deploy-meta">{card.kind}</Box>
        </Box>
        <Box className="cview-deploy-actions">
          <Tooltip title="Open in new tab">
            <span>
              <IconButton
                size="small"
                disabled={!payload.url}
                onClick={() => payload.url && window.open(payload.url, "_blank", "noopener,noreferrer")}
                aria-label="Open deployed app"
              >
                <OpenInNewRounded fontSize="small" />
              </IconButton>
            </span>
          </Tooltip>
          <Tooltip title={copied ? "Copied" : "Copy URL"}>
            <span>
              <IconButton
                size="small"
                disabled={!payload.url}
                onClick={handleCopy}
                aria-label="Copy deploy URL"
              >
                <ContentCopyRounded fontSize="small" />
              </IconButton>
            </span>
          </Tooltip>
        </Box>
      </Box>

      {payload.url ? (
        <a
          className="cview-deploy-url"
          href={payload.url}
          target="_blank"
          rel="noopener noreferrer"
        >
          {payload.url}
        </a>
      ) : (
        <Typography variant="body2" className="cview-deploy-url">
          Deploy URL not yet available.
        </Typography>
      )}

      <Box className="cview-deploy-files-head">
        <Typography component="span" variant="caption">Files</Typography>
        <Typography component="span" variant="caption">{files.length}</Typography>
      </Box>
      {files.length === 0 ? (
        <Typography variant="body2">No files captured for this deploy.</Typography>
      ) : (
        <Box className="cview-deploy-files" role="list">
          {files.map((file, idx) => {
            const size = formatKb(file.bytes);
            const open = () => onOpenFile?.(file.path);
            return (
              <Box
                key={`${file.path}-${idx}`}
                className="cview-deploy-file"
                role="listitem"
                onClick={open}
              >
                <InsertDriveFileRounded
                  fontSize="small"
                  className="cview-deploy-file-icon"
                />
                <span className="cview-deploy-file-path" title={file.path}>
                  {file.path}
                </span>
                {size ? (
                  <span className="cview-deploy-file-size">{size}</span>
                ) : null}
                <IconButton
                  size="small"
                  className="cview-deploy-file-open"
                  aria-label={`Open ${file.path}`}
                  onClick={(event) => {
                    event.stopPropagation();
                    open();
                  }}
                >
                  <ChevronRightRounded fontSize="small" />
                </IconButton>
              </Box>
            );
          })}
        </Box>
      )}
    </Box>
  );
}

export default AppDeployView;
