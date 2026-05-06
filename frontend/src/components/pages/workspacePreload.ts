export function normalizeSettingsPreloadTab(
  rawTab?: number | null,
): number | null {
  if (typeof rawTab !== "number" || !Number.isFinite(rawTab)) return null;
  const tab = Math.trunc(rawTab);
  if (tab === 2 || tab === 10 || tab === 15) return 20;
  if (tab === 9 || tab === 13 || tab === 17) return 0;
  return tab;
}

const loadedPreloads = new Set<string>();

function preloadOnce(key: string, loader: () => Promise<unknown>): void {
  if (loadedPreloads.has(key)) return;
  loadedPreloads.add(key);
  void loader().catch(() => {
    loadedPreloads.delete(key);
  });
}

export function preloadSettingsShell(): void {
  preloadOnce("settings-shell", () => import("./SettingsPage"));
}

export function preloadSettingsFull(): void {
  preloadOnce("settings-full", () => import("./SettingsPageFull"));
}

export function preloadSettingsTab(rawTab?: number | null): void {
  preloadSettingsShell();
  const tab = normalizeSettingsPreloadTab(rawTab);
  if (tab == null) return;
  if (tab !== 0) {
    preloadSettingsFull();
  }
  switch (tab) {
    case 3:
      preloadOnce("settings-media", () => import("./MediaSettingsPanel"));
      break;
    case 1:
      // Models panel is now statically imported by SettingsPageFull, so no
      // separate preload is needed. Falls through to settings-full preload.
      break;
    case 4:
      preloadOnce("settings-security", () => import("./SettingsSecurityPanel"));
      break;
    case 16:
      preloadOnce("settings-sender-verification", () =>
        import("../SenderVerificationPanel"),
      );
      break;
    case 5:
      preloadOnce("settings-advanced", () => import("./SettingsAdvancedPanel"));
      break;
    case 14:
      preloadOnce("settings-data-lifecycle", () =>
        import("./SettingsDataLifecyclePanel"),
      );
      break;
    case 25:
      preloadOnce("settings-updates", () => import("./SettingsUpdatesPanel"));
      break;
    case 6:
      preloadOnce("settings-observability", () => import("../ObservabilityPanel"));
      break;
    case 8:
    case 20:
    case 21:
      preloadOnce("settings-integrations", () => import("../IntegrationsPanel"));
      break;
    case 11:
      preloadOnce("settings-trace", () => import("./TracePage"));
      break;
    case 12:
      preloadOnce("settings-memory", () => import("./MemoryPage"));
      break;
    case 22:
      preloadOnce("settings-webhooks", () => import("../WebhooksPanel"));
      preloadOnce("settings-quickstart", () => import("../IntegrationQuickstartPanel"));
      break;
    case 23:
      preloadOnce("settings-plugins", () => import("../PluginSdkPanel"));
      break;
    case 26:
      preloadOnce("settings-devices", () => import("../CompanionDevicesPanel"));
      break;
    default:
      break;
  }
}
