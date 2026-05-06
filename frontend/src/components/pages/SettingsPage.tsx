import { Box, Stack, Typography } from "@mui/material";
import { useQueryClient } from "@tanstack/react-query";
import { lazy, Suspense, useEffect } from "react";
import { WorkspacePageHeader, WorkspacePageShell } from "../WorkspacePage";
import {
  getSettingsPageMeta,
  getSettingsTabLoadingMessage,
  resolveInitialSettingsTab,
  type SettingsPageProps,
} from "./settingsMeta";
import {
  getSelectedSettingsNav,
  SettingsNavigation,
} from "./settingsNavigation";
import { prefetchSettingsTabData } from "./settingsData";
import { preloadSettingsFull, preloadSettingsTab } from "./workspacePreload";

const SettingsPageFull = lazy(() => import("./SettingsPageFull"));

function SettingsShellFallback({
  tab,
  hideSettingsNav,
}: {
  tab: number;
  hideSettingsNav?: boolean;
}) {
  const selectedSettingsNav = getSelectedSettingsNav(tab, 0);
  const selectedSettingsMeta = getSettingsPageMeta(tab);
  const selectedSettingsHeaderTitle =
    selectedSettingsMeta.title || selectedSettingsNav?.label || "Settings";
  const message = getSettingsTabLoadingMessage(tab);

  return (
    <WorkspacePageShell spacing={1.35}>
      <Box
        className="settings-shell-layout"
        sx={{
          flex: 1,
          minHeight: 0,
          ...(hideSettingsNav
            ? { gridTemplateColumns: "1fr !important" }
            : undefined),
        }}
      >
        {!hideSettingsNav ? (
          <SettingsNavigation tab={tab} onTabChange={() => undefined} />
        ) : null}
        <Box
          className={`settings-main${hideSettingsNav ? " settings-main-standalone" : ""}`}
        >
          <Stack spacing={2} className="workspace-page-shell settings-page-shell">
            <WorkspacePageHeader
              eyebrow={selectedSettingsMeta.kicker}
              title={selectedSettingsHeaderTitle}
              description={selectedSettingsMeta.description}
              className="settings-page-header"
            />
            <Box className="list-shell" sx={{ minHeight: 220, p: 1.5 }}>
              <Typography variant="body2" sx={{ color: "text.secondary" }}>
                {message}
              </Typography>
            </Box>
          </Stack>
        </Box>
      </Box>
    </WorkspacePageShell>
  );
}

export default function SettingsPage({
  autoRefresh,
  initialTab,
  hideSettingsNav,
  standaloneSurface,
}: SettingsPageProps) {
  const queryClient = useQueryClient();
  const resolvedInitialTab = resolveInitialSettingsTab(initialTab);

  useEffect(() => {
    if (standaloneSurface === "arkpulse") return;
    preloadSettingsFull();
    preloadSettingsTab(resolvedInitialTab);
    prefetchSettingsTabData(queryClient, resolvedInitialTab);
  }, [queryClient, standaloneSurface, resolvedInitialTab]);

  return (
    <Suspense
      fallback={
        <SettingsShellFallback
          tab={resolvedInitialTab}
          hideSettingsNav={hideSettingsNav}
        />
      }
    >
      <SettingsPageFull
        autoRefresh={autoRefresh}
        initialTab={resolvedInitialTab}
        hideSettingsNav={hideSettingsNav}
        standaloneSurface={standaloneSurface}
      />
    </Suspense>
  );
}
