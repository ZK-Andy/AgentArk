import { NeuralPanel } from "./NeuralPanel";
import icon1Password from "../../assets/icons/1password.svg";
import iconDiscord from "../../assets/icons/discord.svg";
import iconGarmin from "../../assets/icons/garmin.svg";
import iconGithub from "../../assets/icons/github.svg";
import iconGoogle from "../../assets/icons/google.svg";
import iconGoogleAnalytics from "../../assets/icons/googleanalytics.svg";
import iconGoogleMaps from "../../assets/icons/googlemaps.svg";
import iconGoogleSearchConsole from "../../assets/icons/googlesearchconsole.svg";
import iconJira from "../../assets/icons/jira.svg";
import iconLinear from "../../assets/icons/linear.svg";
import iconMatrix from "../../assets/icons/matrix.svg";
import iconNotion from "../../assets/icons/notion.svg";
import iconSentry from "../../assets/icons/sentry.svg";
import iconShopify from "../../assets/icons/shopify.svg";
import iconSlack from "../../assets/icons/slack.svg";
import iconTeams from "../../assets/icons/teams.svg";
import iconTelegram from "../../assets/icons/telegram.svg";
import iconTwitter from "../../assets/icons/twitter.svg";
import iconWebsearch from "../../assets/icons/websearch.svg";
import iconWhatsapp from "../../assets/icons/whatsapp.svg";

export type ConnectedIntegration = {
  id: string;
  title: string;
  subtitle?: string | null;
  status: string;
  detail?: string | null;
  enabled?: boolean | null;
  connected?: boolean | null;
};

export type AutomationPostureCardProps = {
  automationCounts: {
    tasks: number;
    watchers: number;
    apps: number;
    integrations: number;
  };
  activeIntegrations?: ConnectedIntegration[];
  onOpenInventory?: () => void;
};

const INTEGRATION_ICON_MAP: Record<string, string> = {
  "1password": icon1Password,
  onepassword: icon1Password,
  calendar: iconGoogle,
  discord: iconDiscord,
  garmin: iconGarmin,
  github: iconGithub,
  gmail: iconGoogle,
  google: iconGoogle,
  google_analytics: iconGoogleAnalytics,
  google_calendar: iconGoogle,
  google_places: iconGoogleMaps,
  google_search_console: iconGoogleSearchConsole,
  google_workspace: iconGoogle,
  jira: iconJira,
  linear: iconLinear,
  matrix: iconMatrix,
  notion: iconNotion,
  sentry: iconSentry,
  shopify: iconShopify,
  slack: iconSlack,
  teams: iconTeams,
  telegram: iconTelegram,
  twitter: iconTwitter,
  web_search: iconWebsearch,
  websearch: iconWebsearch,
  whatsapp: iconWhatsapp,
};

function normalizeIntegrationKey(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/&/g, "and")
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
}

function iconForIntegration(item: ConnectedIntegration): string | null {
  const candidates = [
    normalizeIntegrationKey(item.id),
    normalizeIntegrationKey(item.title),
    normalizeIntegrationKey(item.subtitle || ""),
  ].filter(Boolean);

  for (const key of candidates) {
    if (INTEGRATION_ICON_MAP[key]) return INTEGRATION_ICON_MAP[key];
    if (key.includes("google") || key.includes("gmail") || key.includes("calendar")) return iconGoogle;
    if (key.includes("web_search") || key.includes("search")) return iconWebsearch;
  }

  return null;
}

function IntegrationMark({ item }: { item: ConnectedIntegration }) {
  const icon = iconForIntegration(item);
  if (icon) {
    return (
      <span className="nw-integration-mark">
        <img src={icon} alt="" aria-hidden="true" />
      </span>
    );
  }

  const letter = (item.title || item.id || "?").trim().charAt(0).toUpperCase() || "?";
  return <span className="nw-integration-mark nw-integration-mark--fallback">{letter}</span>;
}

export function AutomationPostureCard({
  automationCounts,
  activeIntegrations = [],
  onOpenInventory,
}: AutomationPostureCardProps) {
  const activeCount = activeIntegrations.length;
  const visibleIntegrations = activeIntegrations.slice(0, 12);
  const hiddenCount = Math.max(0, activeCount - visibleIntegrations.length);

  return (
    <NeuralPanel title="Connected Integrations" tag={`${activeCount} ACTIVE`} className="nw-panel--automation">
      {activeCount > 0 ? (
        <div className="nw-integration-strip" aria-label="Active integrations">
          {visibleIntegrations.map((item) => (
            <span
              className="nw-integration-chip"
              key={`${item.id}-${item.title}`}
              title={`${item.title} - ${item.detail && item.detail !== "Enabled" ? item.detail : "Connected and enabled"}`}
            >
              <IntegrationMark item={item} />
            </span>
          ))}
          {hiddenCount > 0 ? <span className="nw-integration-more">+{hiddenCount}</span> : null}
        </div>
      ) : (
        <div className="nw-panel-muted nw-empty-state">
          No active integrations are connected yet.
        </div>
      )}
      <div className="nw-integration-footer">
        <span>{automationCounts.apps} apps</span>
        <span>{automationCounts.watchers} watchers</span>
        {onOpenInventory ? (
          <button
            type="button"
            className="nw-btn nw-btn--small"
            onClick={onOpenInventory}
          >
            Manage <span className="nw-arrow">-&gt;</span>
          </button>
        ) : null}
      </div>
    </NeuralPanel>
  );
}
