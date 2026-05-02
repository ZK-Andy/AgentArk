// Compact step list representing each tool action in an assistant turn.
// Activation drives the right-side Computer pane.
//
// Click handler is provided by the parent (ChatPage).

import type { ComponentType } from "react";
import TerminalRoundedIcon from "@mui/icons-material/TerminalRounded";
import DescriptionRoundedIcon from "@mui/icons-material/DescriptionRounded";
import LanguageRoundedIcon from "@mui/icons-material/LanguageRounded";
import SearchRoundedIcon from "@mui/icons-material/SearchRounded";
import AutoAwesomeRoundedIcon from "@mui/icons-material/AutoAwesomeRounded";
import CheckCircleRoundedIcon from "@mui/icons-material/CheckCircleRounded";
import CancelRoundedIcon from "@mui/icons-material/CancelRounded";
import CircularProgress from "@mui/material/CircularProgress";

import type { ChatStepCard, ComputerViewKind } from "./types";
import {
  chipStatusFromCard,
  extractFilePath,
  extractUrl,
  extractCommand,
  pickComputerView,
  prepareChipCards,
} from "./dispatch";

const VIEW_ICON: Record<
  ComputerViewKind,
  ComponentType<{ fontSize?: "inherit" | "small" | "medium" | "large" }>
> = {
  terminal: TerminalRoundedIcon,
  file: DescriptionRoundedIcon,
  browse: LanguageRoundedIcon,
  search: SearchRoundedIcon,
  app_deploy: TerminalRoundedIcon,
  status: AutoAwesomeRoundedIcon,
};

function urlHost(url: string): string {
  if (!url) return "";
  try {
    const u = new URL(url);
    return u.host.replace(/^www\./, "") + (u.pathname && u.pathname !== "/" ? u.pathname : "");
  } catch {
    return url;
  }
}

function chipTarget(card: ChatStepCard, view: ComputerViewKind): string {
  if (view === "terminal") return extractCommand(card);
  if (view === "app_deploy") return urlHost(extractUrl(card)) || extractCommand(card);
  if (view === "file") return extractFilePath(card);
  if (view === "browse") return urlHost(extractUrl(card));
  if (view === "search") return (card.detail || "").split(/\r?\n/)[0]?.trim() || "";
  return "";
}

function truncate(text: string, max = 36): string {
  if (!text) return "";
  return text.length > max ? `${text.slice(0, max - 3).trimEnd()}...` : text;
}

// Strip the verbose "Running ", "Done ", "Issue " prefixes that the activity
// humanizer inserts. The chip already communicates run state via the left
// accent stripe + spinner — the leading verb just makes labels feel shouty.
// Intent-shaped: we don't match a curated list of tool names, only the
// structural prefix the humanizer adds in front of any tool label.
function cleanChipLabel(label: string): string {
  const trimmed = label.trim();
  const lower = trimmed.toLowerCase();
  for (const prefix of ["running ", "done ", "issue "]) {
    if (lower.startsWith(prefix)) {
      return trimmed.slice(prefix.length).trim() || trimmed;
    }
  }
  return trimmed;
}

export interface ActionChipsProps {
  cards: ChatStepCard[];
  activeStepId?: string | null;
  onActivate?: (id: string) => void;
  live?: boolean;
  keyPrefix?: string;
  maxItems?: number;
}

export function ActionChips({
  cards,
  activeStepId,
  onActivate,
  live,
  keyPrefix = "chip",
  maxItems = 4,
}: ActionChipsProps) {
  if (!cards || cards.length === 0) return null;
  const display = prepareChipCards(cards).slice(-Math.max(1, maxItems));
  if (display.length === 0) return null;
  return (
    <div className="action-chips" role="list" aria-label="Tool actions">
      {display.map((card, idx) => {
        const isLast = idx === display.length - 1;
        const status = chipStatusFromCard(
          card,
          Boolean(live) && isLast,
          Boolean(live),
        );
        const view = pickComputerView(card);
        const Icon = VIEW_ICON[view];
        const isActive = card.id === activeStepId;
        const target = truncate(chipTarget(card, view), 40);
        const tooltip = card.summary || card.detail || card.label;
        return (
          <button
            key={`${keyPrefix}-${card.id}`}
            type="button"
            role="listitem"
            className={[
              "action-chip",
              `status-${status}`,
              `view-${view}`,
              isActive ? "is-active" : "",
            ]
              .filter(Boolean)
              .join(" ")}
            onClick={() => onActivate?.(card.id)}
            title={tooltip}
            aria-label={`${card.label}${target ? ` ${target}` : ""}`}
          >
            <span className="action-chip-icon" aria-hidden="true">
              <Icon fontSize="inherit" />
            </span>
            <span className="action-chip-text">
              <span className="action-chip-primary">{cleanChipLabel(card.label)}</span>
              {target ? (
                <span className="action-chip-secondary">
                  <span className="action-chip-sep" aria-hidden="true">|</span>
                  {target}
                </span>
              ) : null}
            </span>
            <span className="action-chip-status" aria-hidden="true">
              {status === "running" ? (
                <CircularProgress
                  size={11}
                  thickness={6}
                  className="action-chip-spinner"
                />
              ) : status === "done" ? (
                <CheckCircleRoundedIcon
                  fontSize="inherit"
                  className="action-chip-status-icon is-done"
                />
              ) : status === "issue" ? (
                <CancelRoundedIcon
                  fontSize="inherit"
                  className="action-chip-status-icon is-issue"
                />
              ) : null}
            </span>
          </button>
        );
      })}
    </div>
  );
}

export default ActionChips;
