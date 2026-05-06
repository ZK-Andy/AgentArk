import { lazy } from "react";

// Genuinely heavy panels stay lazy: IntegrationsPanel is ~58kb gzipped
// (lots of provider config), and MemoryPage / TracePage are separate routes.
// All other settings panels are statically imported by SettingsPageFull so
// switching tabs inside the Settings dialog never forces a Suspense round.

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

export const MemoryPage = lazy(() => import("./MemoryPage"));

export const TracePage = lazy(() => import("./TracePage"));
