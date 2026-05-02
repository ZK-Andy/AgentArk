import { lazy } from "react";

export const CompanionDevicesPanel = lazy(() =>
  import("../CompanionDevicesPanel").then((module) => ({
    default: module.CompanionDevicesPanel,
  })),
);

export const IntegrationQuickstartPanel = lazy(() =>
  import("../IntegrationQuickstartPanel").then((module) => ({
    default: module.IntegrationQuickstartPanel,
  })),
);

export const IntegrationsPanel = lazy(() =>
  import("../IntegrationsPanel").then((module) => ({
    default: module.IntegrationsPanel,
  })),
);

export const MediaSettingsPanel = lazy(() =>
  import("./MediaSettingsPanel").then((module) => ({
    default: module.MediaSettingsPanel,
  })),
);

export const SettingsAdvancedPanel = lazy(() =>
  import("./SettingsAdvancedPanel").then((module) => ({
    default: module.SettingsAdvancedPanel,
  })),
);

export const MemoryPage = lazy(() => import("./MemoryPage"));

export const ObservabilityPanel = lazy(() =>
  import("../ObservabilityPanel").then((module) => ({
    default: module.ObservabilityPanel,
  })),
);

export const SettingsModelsPanel = lazy(() =>
  import("./SettingsModelsPanel").then((module) => ({
    default: module.SettingsModelsPanel,
  })),
);

export const SettingsSecurityPanel = lazy(() =>
  import("./SettingsSecurityPanel").then((module) => ({
    default: module.SettingsSecurityPanel,
  })),
);

export const PluginSdkPanel = lazy(() =>
  import("../PluginSdkPanel").then((module) => ({
    default: module.PluginSdkPanel,
  })),
);

export const SettingsDataLifecyclePanel = lazy(() =>
  import("./SettingsDataLifecyclePanel").then((module) => ({
    default: module.SettingsDataLifecyclePanel,
  })),
);

export const TracePage = lazy(() => import("./TracePage"));

export const SettingsUpdatesPanel = lazy(() =>
  import("./SettingsUpdatesPanel").then((module) => ({
    default: module.SettingsUpdatesPanel,
  })),
);

export const WebhooksPanel = lazy(() =>
  import("../WebhooksPanel").then((module) => ({
    default: module.WebhooksPanel,
  })),
);
