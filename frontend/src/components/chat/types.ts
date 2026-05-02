// Shared types for the split chat layout (chips + Computer pane).
// Structurally compatible with ActivityTimelineCard from ChatPage.tsx so
// existing card arrays satisfy these interfaces without conversion.

export interface ChatPayloadView {
  kind: "json" | "text";
  badgeLabel: string;
  headerLabel: string;
  preview: string;
  body: string;
  lineCount: number;
}

export interface ChatStepCard {
  id: string;
  index: number;
  stepType: string;
  rawTitle: string;
  tone: string;
  kind: string;
  label: string;
  detail: string;
  detailFull: string;
  summary: string;
  rawDetailFull: string;
  payloadView: ChatPayloadView | null;
  isHeartbeat: boolean;
  time: string;
}

export interface ComputerPaneFile {
  path: string;
  content: string;
}

export type ComputerViewKind =
  | "terminal"
  | "file"
  | "browse"
  | "search"
  | "app_deploy"
  | "status";

export type ComputerPaneTab = "computer" | "activity" | "trace";

export type ChipStatus = "running" | "done" | "issue" | "idle";
