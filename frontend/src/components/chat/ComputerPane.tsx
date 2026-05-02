// Right-side "Computer" pane: one focused live artifact surface with compact
// activity and trace tabs. This is intentionally closer to a runtime console
// than a second copy of the chat timeline.

import { useEffect, useMemo, useState, type ReactNode } from "react";
import Box from "@mui/material/Box";
import Stack from "@mui/material/Stack";
import Typography from "@mui/material/Typography";
import IconButton from "@mui/material/IconButton";
import Tooltip from "@mui/material/Tooltip";
import CloseIcon from "@mui/icons-material/Close";
import ChevronLeftRoundedIcon from "@mui/icons-material/ChevronLeftRounded";
import ChevronRightRoundedIcon from "@mui/icons-material/ChevronRightRounded";
import FiberManualRecordRoundedIcon from "@mui/icons-material/FiberManualRecordRounded";
import ComputerRoundedIcon from "@mui/icons-material/ComputerRounded";

import type { ChatStepCard, ComputerPaneFile, ComputerPaneTab } from "./types";
import { extractFilePath, pickComputerView, prepareChipCards } from "./dispatch";
import {
  AppDeployView,
  BrowseView,
  FileView,
  SearchView,
  StatusView,
  TerminalView,
  TraceTimeline,
  WorkingView,
} from "./computerViews";

export interface ComputerPaneProps {
  liveCards: ChatStepCard[];
  allCards: ChatStepCard[];
  activeStepId: string | null;
  onActivate: (id: string | null) => void;
  onClose: () => void;
  /** Optional rendered node for the Activity tab (e.g. existing classic timeline). */
  activityNode?: ReactNode;
  /** Status text used as fallback heading when no step is active yet. */
  nowDoingLabel?: string;
  /** Active workspace snippet path/content (used by FileView when relevant). */
  snippetPath?: string;
  snippetContent?: string;
  isStreaming?: boolean;
  startedAt?: string | number | null;
  tokenPreview?: string;
  /** Live planner/classifier reasoning text. Surfaced by `WorkingView` as
   * a fallback while the assistant content stream has not started yet. */
  reasoningPreview?: string;
  /** Structural reasoning phase such as "classifier", "planner", or "model". */
  reasoningPhase?: string;
  taskProgress?: {
    done: number;
    total: number;
  } | null;
  showSnippet?: boolean;
  workspaceFiles?: ComputerPaneFile[];
  /** Path of a file currently being written by the agent, if any. When set
   * and the user has not manually picked a different file, the pane will
   * auto-focus this file and stream its content live. */
  liveWritePath?: string | null;
  /** Latest streamed body for `liveWritePath`. Updates while the agent
   * generates the file token-by-token. */
  liveWriteContent?: string;
  /** True while `liveWritePath` is still being written. Once the write is
   * complete the pane stops auto-following and the user can navigate freely. */
  liveWriteActive?: boolean;
}

function pickActiveCard(
  pool: ChatStepCard[],
  activeStepId: string | null,
): ChatStepCard | null {
  if (!pool || pool.length === 0) return null;
  if (activeStepId) {
    const found = pool.find((c) => c.id === activeStepId);
    if (found) return found;
  }
  for (let i = pool.length - 1; i >= 0; i -= 1) {
    if (!pool[i].isHeartbeat) return pool[i];
  }
  return pool[pool.length - 1] ?? null;
}

function ActivityList({
  cards,
  activeStepId,
  onActivate,
}: {
  cards: ChatStepCard[];
  activeStepId: string | null;
  onActivate: (id: string) => void;
}) {
  if (!cards || cards.length === 0) {
    return (
      <Box className="computer-pane-activity-empty">
        <Typography variant="body2" className="computer-pane-activity-empty-copy">
          No activity yet. When AgentArk runs a tool, the steps land here.
        </Typography>
      </Box>
    );
  }
  return (
    <ol className="computer-pane-activity-list">
      {cards.map((card) => {
        const isActive = card.id === activeStepId;
        const time = card.time || "";
        return (
          <li
            key={`activity-${card.id}`}
            className={`computer-pane-activity-row tone-${card.tone}${isActive ? " is-active" : ""}`}
          >
            <button
              type="button"
              className="computer-pane-activity-button"
              onClick={() => onActivate(card.id)}
            >
              <span className="computer-pane-activity-kind">
                {card.kind || "Update"}
              </span>
              <span className="computer-pane-activity-label">{card.label}</span>
              {card.summary || card.detail ? (
                <span className="computer-pane-activity-detail">
                  {card.summary || card.detail}
                </span>
              ) : null}
              {time ? (
                <span className="computer-pane-activity-time">{time}</span>
              ) : null}
            </button>
          </li>
        );
      })}
    </ol>
  );
}

function normalizePath(value: string): string {
  return (value || "")
    .trim()
    .replace(/\\/g, "/")
    .replace(/^\/+/, "")
    .toLowerCase();
}

function str(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function tryParseRecord(raw: string): Record<string, unknown> | null {
  const trimmed = (raw || "").trim();
  if (!trimmed || !trimmed.startsWith("{")) return null;
  try {
    const parsed = JSON.parse(trimmed) as unknown;
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
}

function structuredCardRecord(card: ChatStepCard): Record<string, unknown> | null {
  return (
    tryParseRecord(card.payloadView?.body || "") ||
    tryParseRecord(card.rawDetailFull || "") ||
    tryParseRecord(card.detailFull || "") ||
    null
  );
}

function isReasoningOnlyCard(card: ChatStepCard): boolean {
  const record = structuredCardRecord(card);
  const kind = str(record?.kind, "").trim().toLowerCase();
  const phase = str(record?.phase, "").trim();
  const stepType = (card.stepType || "").trim().toLowerCase();
  if (kind === "reasoning_delta" || stepType === "reasoning_delta") {
    return true;
  }
  return Boolean(
    phase &&
      record &&
      !str(record.tool_name, "") &&
      !str(record.name, "") &&
      !str(record.file, "") &&
      !str(record.path, "") &&
      (str(record.content, "") ||
        str(record.content_delta, "") ||
        str(record.content_snapshot, "")),
  );
}

// The agent runtime wraps every progress / streaming step in an envelope
// shaped like `{flow_kind, tool_name, run_id, seq, ts, content, ...}` where
// `content` is a short progress message ("Drafting vite.config.ts"), NOT the
// file body. If we naively used that envelope as file content the FileView
// would render the wrapper JSON to the user. Detect & refuse it.
const STEP_ENVELOPE_KEYS = ["flow_kind", "tool_name", "run_id", "seq", "ts"];
function isStepEnvelope(record: Record<string, unknown>): boolean {
  let hits = 0;
  for (const key of STEP_ENVELOPE_KEYS) {
    if (key in record) hits += 1;
    if (hits >= 3) return true;
  }
  return false;
}

function pickCardBody(card: ChatStepCard): string {
  return (
    card.payloadView?.body ||
    card.rawDetailFull ||
    card.detailFull ||
    card.detail ||
    ""
  );
}

function contentFromRecordForPath(
  record: Record<string, unknown>,
  targetPath: string,
): string {
  const target = normalizePath(targetPath);
  const recordPath = normalizePath(
    str(record.path, str(record.file, str(record.name, ""))),
  );
  const directContent = str(
    record.raw_content,
    str(record.file_content, str(record.content_snapshot, str(record.content, ""))),
  );
  if (recordPath && (recordPath === target || recordPath.endsWith(`/${target}`))) {
    return directContent;
  }
  const files = record.files;
  if (files && typeof files === "object" && !Array.isArray(files)) {
    for (const [path, content] of Object.entries(files as Record<string, unknown>)) {
      const normalized = normalizePath(path);
      if (normalized === target || normalized.endsWith(`/${target}`)) {
        return str(content, "");
      }
    }
  }
  return "";
}

function findFileContentForPath(cards: ChatStepCard[], path: string): string {
  const target = normalizePath(path);
  if (!target) return "";
  for (let idx = cards.length - 1; idx >= 0; idx -= 1) {
    const card = cards[idx];
    const body = pickCardBody(card);
    const parsed = tryParseRecord(body);
    // Step envelopes never carry the file body in a useful form; their
    // `content` is a progress string. Skip them so we don't fall back to
    // rendering the envelope JSON as if it were the file.
    if (parsed && isStepEnvelope(parsed)) continue;
    if (parsed) {
      const parsedContent = contentFromRecordForPath(parsed, path);
      if (parsedContent.trim()) return parsedContent;
    }
    const cardPath = normalizePath(extractFilePath(card));
    if (cardPath && (cardPath === target || cardPath.endsWith(`/${target}`))) {
      // Only return the raw body when it doesn't parse as JSON. A parseable
      // body without an inner content/files match is almost always a wrapper,
      // never the file we want.
      if (!parsed) return body;
    }
  }
  return "";
}

function findWorkspaceFileContent(
  files: ComputerPaneFile[],
  path: string,
): string {
  const target = normalizePath(path);
  if (!target) return "";
  for (let idx = files.length - 1; idx >= 0; idx -= 1) {
    const file = files[idx];
    const filePath = normalizePath(file.path);
    if (filePath && (filePath === target || filePath.endsWith(`/${target}`))) {
      return file.content || "";
    }
  }
  return "";
}

function filePathsMatch(left: string, right: string): boolean {
  const lhs = normalizePath(left);
  const rhs = normalizePath(right);
  if (!lhs || !rhs) return false;
  return lhs === rhs || lhs.endsWith(`/${rhs}`) || rhs.endsWith(`/${lhs}`);
}

function syntheticFileCard(source: ChatStepCard, path: string): ChatStepCard {
  return {
    ...source,
    id: `${source.id}:file:${path}`,
    stepType: "file_read",
    kind: "File",
    label: path,
    detail: "",
    detailFull: "",
    rawDetailFull: "",
    summary: "",
    payloadView: null,
  };
}

function progressLabel(done: number, total: number): string {
  if (total <= 0) return "Task Progress";
  return `Task Progress ${done}/${total}`;
}

export function ComputerPane({
  liveCards,
  allCards,
  activeStepId,
  onActivate,
  onClose,
  activityNode,
  nowDoingLabel,
  snippetPath,
  snippetContent,
  isStreaming,
  startedAt,
  tokenPreview,
  reasoningPreview,
  reasoningPhase,
  taskProgress = null,
  showSnippet,
  workspaceFiles = [],
  liveWritePath = null,
  liveWriteContent = "",
  liveWriteActive = false,
}: ComputerPaneProps) {
  const [tab, setTab] = useState<ComputerPaneTab>("computer");
  const [deployFilePath, setDeployFilePath] = useState<string | null>(null);
  const [userPickedDeployFile, setUserPickedDeployFile] = useState(false);
  const autoFocusFilePath = liveWritePath || workspaceFiles[0]?.path || null;

  // While a file is actively being written, auto-focus it in the pane so the
  // user watches the code stream in (Bolt/Lovable-style) without having to
  // click. Stops following once the user picks a different file manually,
  // and re-engages on the next live write. When restored after a refresh,
  // keep the last captured workspace file open even if the write has finished.
  useEffect(() => {
    if (!autoFocusFilePath) return;
    if (userPickedDeployFile) return;
    if (!liveWriteActive && deployFilePath) return;
    setDeployFilePath(autoFocusFilePath);
  }, [
    autoFocusFilePath,
    deployFilePath,
    liveWriteActive,
    userPickedDeployFile,
  ]);
  useEffect(() => {
    if (!liveWriteActive) setUserPickedDeployFile(false);
  }, [liveWriteActive]);

  const cardsForRun = useMemo(
    () => (liveCards.length > 0 ? liveCards : allCards),
    [liveCards, allCards],
  );
  const traceCards = useMemo(
    () => cardsForRun.filter((card) => !card.isHeartbeat),
    [cardsForRun],
  );
  const primaryTraceCards = useMemo(
    () => traceCards.filter((card) => !isReasoningOnlyCard(card)),
    [traceCards],
  );
  const navPool = useMemo(
    () =>
      prepareChipCards(cardsForRun).filter(
        (card) => pickComputerView(card) !== "status",
      ),
    [cardsForRun],
  );

  const activeCard = useMemo(
    () => pickActiveCard(navPool, activeStepId),
    [navPool, activeStepId],
  );
  const latestTraceCard = useMemo(
    () => pickActiveCard(primaryTraceCards, activeStepId),
    [primaryTraceCards, activeStepId],
  );

  const activeIndex = useMemo(
    () => (activeCard ? navPool.findIndex((c) => c.id === activeCard.id) : -1),
    [navPool, activeCard],
  );
  const canPrev = activeIndex > 0;
  const canNext = activeIndex >= 0 && activeIndex < navPool.length - 1;
  const view = activeCard ? pickComputerView(activeCard) : "status";
  const headerText = activeCard?.label || nowDoingLabel || "Working";
  const completedCount = navPool.filter((card) =>
    /done|complete|success/i.test(card.kind || ""),
  ).length;
  const progressText =
    taskProgress && taskProgress.total > 0
      ? progressLabel(
          Math.max(0, Math.min(taskProgress.done, taskProgress.total)),
          taskProgress.total,
        )
      : progressLabel(
          Math.max(completedCount, activeIndex + 1, 0),
          navPool.length,
        );
  const deployFileIsLiveWrite =
    !!deployFilePath &&
    !!liveWritePath &&
    filePathsMatch(deployFilePath, liveWritePath);
  const deployFileContent = useMemo(() => {
    if (!deployFilePath) return "";
    // While the file is streaming, prefer the live buffer over any captured
    // workspace snapshot so the user sees the just-written line, not stale
    // content from a previous run.
    if (deployFileIsLiveWrite && liveWriteContent) return liveWriteContent;
    return (
      findWorkspaceFileContent(workspaceFiles, deployFilePath) ||
      findFileContentForPath(cardsForRun, deployFilePath)
    );
  }, [
    cardsForRun,
    deployFilePath,
    deployFileIsLiveWrite,
    liveWriteContent,
    workspaceFiles,
  ]);
  const activeFilePath =
    activeCard && view === "file"
      ? extractFilePath(activeCard) || activeCard.label || ""
      : "";
  const activeFileIsLiveWrite =
    !!activeFilePath &&
    !!liveWritePath &&
    filePathsMatch(activeFilePath, liveWritePath);
  const activeFileContent = useMemo(() => {
    if (!activeFilePath) return "";
    if (activeFileIsLiveWrite && liveWriteContent) return liveWriteContent;
    return (
      findWorkspaceFileContent(workspaceFiles, activeFilePath) ||
      findFileContentForPath(cardsForRun, activeFilePath)
    );
  }, [
    activeFilePath,
    activeFileIsLiveWrite,
    cardsForRun,
    liveWriteContent,
    workspaceFiles,
  ]);
  const fallbackFilePath =
    !activeCard && (deployFilePath || liveWritePath || workspaceFiles[0]?.path)
      ? deployFilePath || liveWritePath || workspaceFiles[0]?.path || ""
      : "";
  const fallbackFileIsLiveWrite =
    !!fallbackFilePath &&
    !!liveWritePath &&
    filePathsMatch(fallbackFilePath, liveWritePath);
  const fallbackFileContent = useMemo(() => {
    if (!fallbackFilePath) return "";
    if (fallbackFileIsLiveWrite && liveWriteContent) {
      return liveWriteContent;
    }
    return (
      findWorkspaceFileContent(workspaceFiles, fallbackFilePath) ||
      findFileContentForPath(cardsForRun, fallbackFilePath)
    );
  }, [
    cardsForRun,
    fallbackFileIsLiveWrite,
    fallbackFilePath,
    liveWriteContent,
    workspaceFiles,
  ]);
  const fallbackFileSourceCard = latestTraceCard || activeCard;
  const fallbackFileCard =
    fallbackFilePath && fallbackFileSourceCard
      ? syntheticFileCard(fallbackFileSourceCard, fallbackFilePath)
      : fallbackFilePath
        ? syntheticFileCard(
            {
              id: "workspace-file",
              index: 0,
              stepType: "file_read",
              rawTitle: "",
              tone: "default",
              kind: "File",
              label: fallbackFilePath,
              detail: "",
              detailFull: "",
              summary: "",
              rawDetailFull: "",
              payloadView: null,
              isHeartbeat: false,
              time: "",
            },
            fallbackFilePath,
          )
        : null;
  const deployFileCard =
    activeCard && deployFilePath
      ? syntheticFileCard(activeCard, deployFilePath)
      : null;
  const snippetCard =
    showSnippet && (snippetPath || snippetContent)
      ? syntheticFileCard(
          activeCard || latestTraceCard || {
            id: "workspace-snippet",
            index: 0,
            stepType: "file_read",
            rawTitle: "",
            tone: "default",
            kind: "File",
            label: snippetPath || "Code",
            detail: "",
            detailFull: "",
            summary: "",
            rawDetailFull: "",
            payloadView: null,
            isHeartbeat: false,
            time: "",
          },
          snippetPath || "Code",
        )
      : null;

  return (
    <Box
      className="computer-pane"
      sx={{
        display: "flex",
        flexDirection: "column",
        minHeight: 0,
        height: "100%",
      }}
    >
      <Stack
        direction="row"
        spacing={1}
        className="computer-pane-toolbar"
        sx={{ alignItems: "center" }}
      >
        <Box className="computer-pane-title">
          <Box className="computer-pane-brand-row">
            <ComputerRoundedIcon fontSize="small" className="computer-pane-brand-icon" />
            <Typography variant="subtitle2" className="computer-pane-heading">
              AgentArk Computer
            </Typography>
          </Box>
          <Stack
            direction="row"
            spacing={0.8}
            className="computer-pane-progress-row"
            sx={{ alignItems: "center" }}
          >
            <FiberManualRecordRoundedIcon
              fontSize="inherit"
              className={isStreaming ? "computer-pane-live-dot" : "computer-pane-idle-dot"}
            />
            <Typography variant="caption" className="computer-pane-progress">
              {progressText}
            </Typography>
            <span className="computer-pane-step-sep">|</span>
            <Typography variant="caption" className="computer-pane-current-step">
              {headerText}
            </Typography>
          </Stack>
        </Box>
        <Box sx={{ flex: 1 }} />
        <Box className="computer-pane-tabs" role="tablist" aria-label="Console view">
          {(["computer", "activity", "trace"] as ComputerPaneTab[]).map((value) => {
            const active = tab === value;
            return (
              <button
                key={value}
                type="button"
                role="tab"
                aria-selected={active}
                className={`computer-pane-tab${active ? " is-active" : ""}`}
                onClick={() => setTab(value)}
              >
                {value === "computer"
                  ? "Computer"
                  : value === "activity"
                    ? "Activity"
                    : "Trace"}
              </button>
            );
          })}
        </Box>
        <Tooltip title="Close console">
          <IconButton
            size="small"
            aria-label="Close AgentArk Console"
            onClick={onClose}
          >
            <CloseIcon fontSize="small" />
          </IconButton>
        </Tooltip>
      </Stack>

      {tab === "computer" ? (
        <Box
          className="computer-pane-body computer-pane-body-computer"
          sx={{
            flex: 1,
            minHeight: 0,
            display: "flex",
            flexDirection: "column",
            overflow: "hidden",
          }}
        >
          <Stack
            direction="row"
            spacing={0.5}
            className="computer-pane-nav"
            sx={{ alignItems: "center" }}
          >
            <IconButton
              size="small"
              disabled={!canPrev}
              aria-label="Previous artifact"
              onClick={() => {
                if (!canPrev) return;
                setDeployFilePath(null);
                onActivate(navPool[activeIndex - 1].id);
              }}
            >
              <ChevronLeftRoundedIcon fontSize="small" />
            </IconButton>
            <Typography variant="caption" className="computer-pane-nav-pos">
              {activeIndex >= 0 ? `${activeIndex + 1} / ${navPool.length}` : "working"}
            </Typography>
            <IconButton
              size="small"
              disabled={!canNext}
              aria-label="Next artifact"
              onClick={() => {
                if (!canNext) return;
                setDeployFilePath(null);
                onActivate(navPool[activeIndex + 1].id);
              }}
            >
              <ChevronRightRoundedIcon fontSize="small" />
            </IconButton>
            <Box sx={{ flex: 1 }} />
            {isStreaming ? (
              <Stack
                direction="row"
                spacing={0.4}
                sx={{ alignItems: "center" }}
                className="computer-pane-live"
              >
                <FiberManualRecordRoundedIcon
                  fontSize="inherit"
                  className="computer-pane-live-dot"
                />
                <Typography variant="caption" className="computer-pane-live-label">
                  live
                </Typography>
              </Stack>
            ) : null}
            {activeStepId ? (
              <button
                type="button"
                className="computer-pane-follow-button"
                onClick={() => {
                  setDeployFilePath(null);
                  onActivate(null);
                }}
                title="Resume following the latest step"
              >
                Follow latest
              </button>
            ) : null}
          </Stack>
          <Box
            className="computer-pane-stage"
            sx={{ flex: 1, minHeight: 0, overflow: "auto" }}
          >
            {snippetCard ? (
              <FileView
                card={snippetCard}
                snippetPath={snippetPath}
                snippetContent={snippetContent}
              />
            ) : !activeCard && fallbackFileCard ? (
              <FileView
                card={fallbackFileCard}
                snippetPath={fallbackFilePath}
                snippetContent={fallbackFileContent}
                live={
                  fallbackFileIsLiveWrite && liveWriteActive
                }
              />
            ) : !activeCard ? (
              isStreaming || latestTraceCard || reasoningPreview ? (
                <WorkingView
                  phaseLabel={nowDoingLabel || "Working..."}
                  detail={latestTraceCard?.detail || latestTraceCard?.summary || ""}
                  startedAt={startedAt}
                  tokenPreview={tokenPreview}
                  reasoningPreview={reasoningPreview}
                  reasoningPhase={reasoningPhase}
                />
              ) : (
                <StatusView
                  title="Idle"
                  detail="When AgentArk runs a tool, its live output will land here."
                />
              )
            ) : view === "terminal" ? (
              <TerminalView
                card={activeCard}
                live={Boolean(isStreaming) && activeIndex === navPool.length - 1}
              />
            ) : view === "app_deploy" ? (
              <Stack spacing={1}>
                <AppDeployView
                  card={activeCard}
                  workspaceFiles={workspaceFiles}
                  onOpenFile={(path) => {
                    setUserPickedDeployFile(path !== liveWritePath);
                    setDeployFilePath(path);
                  }}
                />
                {deployFileCard && deployFilePath ? (
                  <FileView
                    card={deployFileCard}
                    snippetPath={deployFilePath}
                    snippetContent={deployFileContent}
                    live={deployFileIsLiveWrite && liveWriteActive}
                  />
                ) : null}
              </Stack>
            ) : view === "file" ? (
              <FileView
                card={activeCard}
                snippetPath={activeFilePath || snippetPath}
                snippetContent={activeFileContent}
                live={activeFileIsLiveWrite && liveWriteActive}
              />
            ) : view === "browse" ? (
              <BrowseView card={activeCard} />
            ) : view === "search" ? (
              <SearchView card={activeCard} />
            ) : (
              <StatusView
                title={activeCard.label}
                detail={activeCard.detail || activeCard.summary || ""}
              />
            )}
          </Box>
        </Box>
      ) : tab === "activity" ? (
        <Box
          className="computer-pane-body computer-pane-body-activity"
          sx={{ flex: 1, minHeight: 0, overflow: "auto" }}
        >
          {activityNode || (
            <ActivityList
              cards={traceCards}
              activeStepId={activeStepId}
              onActivate={(id) => {
                onActivate(id);
                setTab("computer");
              }}
            />
          )}
        </Box>
      ) : (
        <Box
          className="computer-pane-body computer-pane-body-trace"
          sx={{ flex: 1, minHeight: 0, overflow: "auto" }}
        >
          <TraceTimeline
            cards={traceCards}
            activeStepId={activeCard?.id || activeStepId}
            onActivate={(id) => {
              onActivate(id);
              setTab("computer");
            }}
          />
        </Box>
      )}
    </Box>
  );
}

export default ComputerPane;
