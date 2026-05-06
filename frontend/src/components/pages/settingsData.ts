import type { QueryClient, QueryKey } from "@tanstack/react-query";
import { api } from "../../api/client";
import { normalizeSettingsTab } from "./settingsMeta";

export const SETTINGS_QUERY_KEYS = {
  settings: ["settings"] as const,
  availableMessagingChannels: ["available-messaging-channels"] as const,
  media: ["settings-media"] as const,
  models: ["models"] as const,
  updateStatus: ["settings-update-status"] as const,
  apiKey: ["settings-api-key"] as const,
  autonomySettings: ["settings-autonomy-settings"] as const,
  evolution: ["settings-evolution"] as const,
  sentinel: ["sentinel-settings"] as const,
  tunnelStatus: ["tunnel-status"] as const,
  tunnelProviders: ["tunnel-providers"] as const,
  securityStatus: ["security-status"] as const,
  securityAbuseReviews: ["security-abuse-reviews"] as const,
  observabilityLogs: ["settings-observability-logs"] as const,
  secrets: ["settings-secrets"] as const,
  arkPulseLog: ["arkpulse-log"] as const,
};

export const CORE_SETTINGS_STALE_TIME_MS = 30_000;
export const SETTINGS_BACKGROUND_STALE_TIME_MS = 60_000;
export const SETTINGS_CACHE_GC_TIME_MS = 15 * 60_000;
export const MODEL_SLOT_INDEX_ID_PREFIX = "__slot_idx_";

export const fetchSettings = () => api.rawGet("/settings");
export const fetchAvailableMessagingChannels = () =>
  api.rawGet("/channels/available");
export const fetchSettingsMedia = () => api.rawGet("/settings/media");
export const fetchModels = () => api.rawGet("/models");
export const fetchSettingsUpdateStatus = () => api.getStatus();
export const fetchSettingsApiKey = () => api.rawGet("/settings/api-key");
export const fetchSettingsAutonomy = () => api.rawGet("/autonomy/settings");
export const fetchSettingsEvolution = () => api.rawGet("/settings/evolution");
export const fetchSettingsSentinel = () =>
  api.rawGet("/autonomy/sentinel/settings");
export const fetchTunnelStatus = () => api.rawGet("/tunnel/status");
export const fetchTunnelProviders = () => api.rawGet("/tunnel/providers");
export const fetchSecurityStatus = () => api.rawGet("/security/status");
export const fetchSecurityAbuseReviews = () =>
  api.rawGet("/security/abuse-reviews");
export const fetchSettingsObservabilityLogs = () =>
  api.rawGet("/settings/observability/logs?limit=40");
export const fetchSettingsSecrets = () => api.rawGet("/settings/secrets");
export const fetchArkPulseLog = () => api.rawGet("/arkpulse?limit=40");

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  return value != null && typeof value === "object" && !Array.isArray(value);
}

export function normalizeModelSlotRows(value: unknown): Record<string, unknown>[] {
  if (!Array.isArray(value)) return [];
  return value.reduce<Record<string, unknown>[]>((rows, item, index) => {
    if (!isPlainRecord(item)) return rows;
    const id = typeof item.id === "string" ? item.id.trim() : "";
    rows.push({
      ...item,
      id: id || `${MODEL_SLOT_INDEX_ID_PREFIX}${index}`,
    });
    return rows;
  }, []);
}

export function modelsPayloadFromSettings(value: unknown): Record<string, unknown> {
  const settings = isPlainRecord(value) ? value : {};
  return {
    models: normalizeModelSlotRows(settings.model_pool),
    smart_routing: Boolean(settings.smart_routing),
  };
}

type SettingsPrefetch = {
  queryKey: QueryKey;
  queryFn: () => Promise<unknown>;
  staleTime: number;
};

function prefetchSettingsQueries(
  queryClient: QueryClient,
  queries: SettingsPrefetch[],
): void {
  const seen = new Set<string>();
  void Promise.allSettled(
    queries
      .filter((query) => {
        const key = JSON.stringify(query.queryKey);
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      })
      .map((query) =>
        queryClient.prefetchQuery({
          ...query,
          gcTime: SETTINGS_CACHE_GC_TIME_MS,
        }),
      ),
  );
}

export function prefetchCoreSettingsData(queryClient: QueryClient): void {
  prefetchSettingsQueries(queryClient, [
    {
      queryKey: SETTINGS_QUERY_KEYS.settings,
      queryFn: fetchSettings,
      staleTime: CORE_SETTINGS_STALE_TIME_MS,
    },
    {
      queryKey: SETTINGS_QUERY_KEYS.availableMessagingChannels,
      queryFn: fetchAvailableMessagingChannels,
      staleTime: CORE_SETTINGS_STALE_TIME_MS,
    },
    {
      queryKey: SETTINGS_QUERY_KEYS.media,
      queryFn: fetchSettingsMedia,
      staleTime: CORE_SETTINGS_STALE_TIME_MS,
    },
  ]);
}

export function prefetchSettingsTabData(
  queryClient: QueryClient,
  rawTab?: number | null,
): void {
  const tab = normalizeSettingsTab(rawTab);
  const queries: SettingsPrefetch[] = [
    {
      queryKey: SETTINGS_QUERY_KEYS.settings,
      queryFn: fetchSettings,
      staleTime: CORE_SETTINGS_STALE_TIME_MS,
    },
  ];

  if (tab === 0 || tab === 20) {
    queries.push({
      queryKey: SETTINGS_QUERY_KEYS.availableMessagingChannels,
      queryFn: fetchAvailableMessagingChannels,
      staleTime: CORE_SETTINGS_STALE_TIME_MS,
    });
  }
  if (tab === 0 || tab === 3) {
    queries.push({
      queryKey: SETTINGS_QUERY_KEYS.media,
      queryFn: fetchSettingsMedia,
      staleTime: CORE_SETTINGS_STALE_TIME_MS,
    });
  }
  if (tab === 4) {
    queries.push(
      {
        queryKey: SETTINGS_QUERY_KEYS.tunnelStatus,
        queryFn: fetchTunnelStatus,
        staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
      },
      {
        queryKey: SETTINGS_QUERY_KEYS.tunnelProviders,
        queryFn: fetchTunnelProviders,
        staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
      },
      {
        queryKey: SETTINGS_QUERY_KEYS.securityStatus,
        queryFn: fetchSecurityStatus,
        staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
      },
      {
        queryKey: SETTINGS_QUERY_KEYS.securityAbuseReviews,
        queryFn: fetchSecurityAbuseReviews,
        staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
      },
      {
        queryKey: SETTINGS_QUERY_KEYS.secrets,
        queryFn: fetchSettingsSecrets,
        staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
      },
    );
  }
  if (tab === 5) {
    queries.push(
      {
        queryKey: SETTINGS_QUERY_KEYS.apiKey,
        queryFn: fetchSettingsApiKey,
        staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
      },
      {
        queryKey: SETTINGS_QUERY_KEYS.autonomySettings,
        queryFn: fetchSettingsAutonomy,
        staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
      },
      {
        queryKey: SETTINGS_QUERY_KEYS.evolution,
        queryFn: fetchSettingsEvolution,
        staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
      },
      {
        queryKey: SETTINGS_QUERY_KEYS.sentinel,
        queryFn: fetchSettingsSentinel,
        staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
      },
    );
  }
  if (tab === 6) {
    queries.push({
      queryKey: SETTINGS_QUERY_KEYS.observabilityLogs,
      queryFn: fetchSettingsObservabilityLogs,
      staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
    });
  }
  if (tab === 9) {
    queries.push({
      queryKey: SETTINGS_QUERY_KEYS.arkPulseLog,
      queryFn: fetchArkPulseLog,
      staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
    });
  }
  if (tab === 25) {
    queries.push({
      queryKey: SETTINGS_QUERY_KEYS.updateStatus,
      queryFn: fetchSettingsUpdateStatus,
      staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
    });
  }

  prefetchSettingsQueries(queryClient, queries);
}

export function prefetchSettingsPageData(queryClient: QueryClient): void {
  prefetchCoreSettingsData(queryClient);
  prefetchSettingsQueries(queryClient, [
    {
      queryKey: SETTINGS_QUERY_KEYS.updateStatus,
      queryFn: fetchSettingsUpdateStatus,
      staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
    },
    {
      queryKey: SETTINGS_QUERY_KEYS.apiKey,
      queryFn: fetchSettingsApiKey,
      staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
    },
    {
      queryKey: SETTINGS_QUERY_KEYS.autonomySettings,
      queryFn: fetchSettingsAutonomy,
      staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
    },
    {
      queryKey: SETTINGS_QUERY_KEYS.evolution,
      queryFn: fetchSettingsEvolution,
      staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
    },
    {
      queryKey: SETTINGS_QUERY_KEYS.sentinel,
      queryFn: fetchSettingsSentinel,
      staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
    },
    {
      queryKey: SETTINGS_QUERY_KEYS.tunnelStatus,
      queryFn: fetchTunnelStatus,
      staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
    },
    {
      queryKey: SETTINGS_QUERY_KEYS.tunnelProviders,
      queryFn: fetchTunnelProviders,
      staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
    },
    {
      queryKey: SETTINGS_QUERY_KEYS.securityStatus,
      queryFn: fetchSecurityStatus,
      staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
    },
    {
      queryKey: SETTINGS_QUERY_KEYS.securityAbuseReviews,
      queryFn: fetchSecurityAbuseReviews,
      staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
    },
    {
      queryKey: SETTINGS_QUERY_KEYS.observabilityLogs,
      queryFn: fetchSettingsObservabilityLogs,
      staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
    },
    {
      queryKey: SETTINGS_QUERY_KEYS.secrets,
      queryFn: fetchSettingsSecrets,
      staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
    },
    {
      queryKey: SETTINGS_QUERY_KEYS.arkPulseLog,
      queryFn: fetchArkPulseLog,
      staleTime: SETTINGS_BACKGROUND_STALE_TIME_MS,
    },
  ]);
}
