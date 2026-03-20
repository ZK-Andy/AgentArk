type JsonRecord = Record<string, unknown>;

function isRecord(value: unknown): value is JsonRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function asRecord(value: unknown): JsonRecord {
  return isRecord(value) ? value : {};
}

function str(value: unknown, fallback = ""): string {
  if (typeof value === "string" && value.trim()) return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return fallback;
}

function toBool(value: unknown): boolean {
  if (typeof value === "boolean") return value;
  if (typeof value === "number") return value !== 0;
  if (typeof value === "string") {
    const normalized = value.trim().toLowerCase();
    return normalized === "true" || normalized === "1" || normalized === "yes";
  }
  return false;
}

export type TunnelAccessMeta = {
  exposure: "public" | "tailnet_private";
  isPrivate: boolean;
  e2ee: boolean;
  linkLabel: string;
  accessNoun: string;
  exposureLabel: string;
  securityLabel: string;
};

export function getTunnelAccessMeta(value: unknown): TunnelAccessMeta {
  const record = asRecord(value);
  const exposure = str(record.exposure, "public").trim().toLowerCase();
  const isPrivate = exposure === "tailnet_private";
  const e2ee = toBool(record.e2ee);
  return {
    exposure: isPrivate ? "tailnet_private" : "public",
    isPrivate,
    e2ee,
    linkLabel: str(record.link_label, isPrivate ? "Private Tailnet URL" : "Public Link"),
    accessNoun: isPrivate ? "private access" : "public link",
    exposureLabel: isPrivate ? "Private tailnet" : "Public internet",
    securityLabel: e2ee ? "End-to-end encrypted" : "Encrypted in transit"
  };
}

export function getTunnelStatusSummary(active: boolean, meta: TunnelAccessMeta): string {
  if (active) {
    return meta.isPrivate ? "Private access is live" : "Public link is live";
  }
  return meta.isPrivate ? "Private access is off" : "Public link is off";
}

export function getTunnelUrlFieldLabel(meta: TunnelAccessMeta): string {
  return meta.linkLabel;
}

export function getTunnelStartButtonLabel(meta: TunnelAccessMeta, hasCustomMasterPassword: boolean): string {
  if (meta.isPrivate) {
    return hasCustomMasterPassword ? "Get Private Access URL" : "Set Password & Get Private Access URL";
  }
  return hasCustomMasterPassword ? "Get Public Link" : "Set Password & Get Public Link";
}

export function getTunnelStopButtonLabel(meta: TunnelAccessMeta): string {
  return meta.isPrivate ? "Stop Private Access" : "Stop Public Link";
}

export function getTunnelPanelStartMessage(meta: TunnelAccessMeta, url: string): string {
  return `${meta.accessNoun === "private access" ? "Private access URL" : "Public link"} is ready. ${url}`;
}

export function getTunnelPanelStartingMessage(meta: TunnelAccessMeta): string {
  return meta.isPrivate
    ? "Private access is starting. The private URL will appear here in a few seconds."
    : "Tunnel is starting. The public link will appear here in a few seconds.";
}

export function getTunnelPanelResumeMessage(meta: TunnelAccessMeta): string {
  return meta.isPrivate
    ? "Custom password saved. Creating your private access URL now..."
    : "Custom password saved. Creating your public link now...";
}

export function getTunnelPanelPasswordPrompt(meta: TunnelAccessMeta): string {
  return meta.isPrivate
    ? "Set a custom AgentArk password first. The private access URL will start right after that."
    : "Set a custom AgentArk password first. The public link will start right after that.";
}

export function getTunnelPanelWarning(meta: TunnelAccessMeta): string {
  return meta.isPrivate
    ? "Anyone with the tailnet URL reaches your AgentArk sign-in page. They still need your custom AgentArk password to get in, and traffic stays end-to-end encrypted over the tailnet."
    : "Anyone with the URL reaches your AgentArk sign-in page. They still need your custom AgentArk password to get in, and you should stop the public link when you no longer need it.";
}

export function getTunnelProviderHelp(meta: TunnelAccessMeta): string {
  return meta.isPrivate
    ? "Private tailnet access uses Tailscale devices you control. It keeps traffic end-to-end encrypted and is not publicly exposed."
    : "This provider exposes a public URL. It is encrypted in transit, but not end-to-end encrypted.";
}

export function getAppShareLinkLabel(meta: TunnelAccessMeta): string {
  return meta.isPrivate ? "Copy Private Tailnet URL" : "Copy Public Link";
}

export function getAppShareOpenLabel(meta: TunnelAccessMeta): string {
  return meta.isPrivate ? "Open Private Access" : "Open Public";
}

export function getAppSharePublicCaption(meta: TunnelAccessMeta): string {
  return meta.isPrivate ? "Private access:" : "Public:";
}
