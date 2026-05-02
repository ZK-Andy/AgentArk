import { Box } from "@mui/material";
import { memo } from "react";
import ChatPage from "./pages/ChatPage";
import { WorkspaceViewOutlet } from "./WorkspaceViewOutlet";
import { type WorkspaceView } from "./workspaceSurface";

export {
  preloadCommonSettingsPanels,
  preloadSettingsTab,
  preloadWorkspaceSurface,
} from "./workspaceSurface";
export type { WorkspaceView } from "./workspaceSurface";

function NativeWorkspaceInner({
  view,
  autoRefresh,
  showAdvanced: _showAdvanced,
  settingsInitialTab,
  onNavigateToView,
}: {
  view: WorkspaceView;
  autoRefresh: boolean;
  showAdvanced: boolean;
  settingsInitialTab?: number | null;
  onNavigateToView?: (view: string, replace?: boolean) => void;
}) {
  const isChat = view === "chat";
  const isSettingsSurface =
    view === "settings" ||
    [
      "connections",
      "channels",
      "routing",
      "webhooks",
      "devices",
      "browser",
      "gatewayops",
      "failover",
      "search",
    ].includes(view);
  return (
    <Box
      sx={{
        p: isChat
          ? { xs: 0.35, md: 0.45 }
          : isSettingsSurface
            ? { xs: 1, md: 1.25 }
            : { xs: 0.75, md: 1 },
        height: "100%",
        overflow: isChat ? "hidden" : "auto",
        display: "flex",
        flexDirection: "column",
        minHeight: 0,
        minWidth: 0,
        width: "100%",
      }}
    >
      <Box
        sx={{
          display: isChat ? "flex" : "none",
          flex: 1,
          minHeight: 0,
          minWidth: 0,
          width: "100%",
        }}
      >
        <ChatPage
          autoRefresh={autoRefresh}
          isActive={isChat}
          onNavigateToView={onNavigateToView}
        />
      </Box>

      <WorkspaceViewOutlet
        view={view}
        autoRefresh={autoRefresh}
        settingsInitialTab={settingsInitialTab}
        onNavigateToView={onNavigateToView}
      />
    </Box>
  );
}

export const NativeWorkspace = memo(NativeWorkspaceInner);
NativeWorkspace.displayName = "NativeWorkspace";
