import AutorenewRoundedIcon from "@mui/icons-material/AutorenewRounded";
import {
  Alert,
  Box,
  Button,
  Chip,
  Link,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import Grid2 from "@mui/material/Grid";
import type { ReactNode } from "react";
import type { StatusResponse, UpdateStatus } from "../../types";
import { errMessage, str } from "./pageHelpers";
import type {
  SettingsInlineCardProps,
  SettingsSectionIntroArgs,
} from "./settingsLayout";

type RestartNotice = {
  text: string;
  etaLabel: string;
};

type SettingsUpdatesPanelProps = {
  restartNotice: RestartNotice | null;
  renderSettingsInlineCard: (props: SettingsInlineCardProps) => ReactNode;
  renderSettingsSectionIntro: (props: SettingsSectionIntroArgs) => ReactNode;
  updateStatus: UpdateStatus | null;
  updateCheckedAtLabel: string | null;
  updateStatusQ: {
    data?: StatusResponse;
    isLoading: boolean;
    error: unknown;
  };
  updateAgentArkPending: boolean;
  onUpdateAndRestart: () => Promise<void>;
};

export function SettingsUpdatesPanel({
  restartNotice,
  renderSettingsInlineCard,
  renderSettingsSectionIntro,
  updateStatus,
  updateCheckedAtLabel,
  updateStatusQ,
  updateAgentArkPending,
  onUpdateAndRestart,
}: SettingsUpdatesPanelProps) {
  return (
    <Stack spacing={2.5}>
      {restartNotice
        ? renderSettingsInlineCard({
            eyebrow: "Restarting",
            title: "AgentArk is coming back online",
            description: restartNotice.text,
            tone: "info",
            action: (
              <Chip
                size="small"
                icon={<AutorenewRoundedIcon />}
                label={restartNotice.etaLabel}
                color="info"
                variant="outlined"
              />
            ),
          })
        : null}

      {renderSettingsInlineCard({
        eyebrow: "Updates",
        title: "Managed release updates",
        description:
          "Updating restarts AgentArk. Pending chats, running jobs, and in-flight approvals can be interrupted. Stored data and conversation history remain on this machine.",
        tone: "warning",
        fullWidthCopy: true,
      })}

      <Box className="list-shell">
        <Stack spacing={2}>
          {renderSettingsSectionIntro({
            eyebrow: "Updates",
            title: "Release status",
            description:
              "Track the installed version and the latest tagged release without polling GitHub on every page refresh.",
            action:
              updateStatus?.state === "available" &&
              updateStatus.apply_supported ? (
                <Button
                  size="small"
                  variant="contained"
                  color="warning"
                  disabled={updateAgentArkPending || !!restartNotice}
                  onClick={() => {
                    void onUpdateAndRestart();
                  }}
                >
                  {updateAgentArkPending ? "Starting..." : "Update and Restart"}
                </Button>
              ) : null,
          })}

          {updateStatusQ.isLoading && !updateStatus ? (
            <Alert severity="info">Checking the latest release.</Alert>
          ) : updateStatusQ.error ? (
            <Alert severity="error">{errMessage(updateStatusQ.error)}</Alert>
          ) : null}

          <Grid2 container spacing={1.5}>
            <Grid2 size={{ xs: 12, md: 6 }}>
              <TextField
                fullWidth
                size="small"
                label="Installed version"
                value={str(updateStatusQ.data?.version, "Unknown")}
                disabled
              />
            </Grid2>
            <Grid2 size={{ xs: 12, md: 6 }}>
              <TextField
                fullWidth
                size="small"
                label="Latest release"
                value={str(updateStatus?.latest_version, "Unavailable")}
                disabled
              />
            </Grid2>
          </Grid2>

          {updateStatus?.checked_at ? (
            <Typography variant="caption" sx={{ color: "text.secondary" }}>
              Last checked {updateCheckedAtLabel}
            </Typography>
          ) : null}

          {updateStatus?.release_url ? (
            <Link
              href={updateStatus.release_url}
              target="_blank"
              rel="noreferrer"
              underline="hover"
            >
              Open release notes
            </Link>
          ) : null}

          {(() => {
            if (!updateStatus) {
              return null;
            }
            if (updateStatus.state === "available") {
              return (
                <Alert severity="warning">
                  A newer tagged release is available.
                  {updateStatus.apply_supported
                    ? " Start the update here when you are ready for a restart."
                    : ` ${updateStatus.apply_message || "Update this deployment from the CLI instead."}`}
                </Alert>
              );
            }
            if (updateStatus.state === "current") {
              return (
                <Alert severity="success">
                  This installation is already on the latest tagged release.
                </Alert>
              );
            }
            if (updateStatus.state === "unavailable") {
              return (
                <Alert severity="info">
                  Update status is unavailable for this deployment. This is
                  expected while release metadata is private or temporarily
                  unreachable.
                </Alert>
              );
            }
            return <Alert severity="info">Checking release metadata.</Alert>;
          })()}
        </Stack>
      </Box>
    </Stack>
  );
}
