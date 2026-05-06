import CheckCircleRoundedIcon from "@mui/icons-material/CheckCircleRounded";
import DoneAllRoundedIcon from "@mui/icons-material/DoneAllRounded";
import HourglassTopRoundedIcon from "@mui/icons-material/HourglassTopRounded";
import SaveRoundedIcon from "@mui/icons-material/SaveRounded";
import ShieldOutlinedIcon from "@mui/icons-material/ShieldOutlined";
import VerifiedUserRoundedIcon from "@mui/icons-material/VerifiedUserRounded";
import {
  Alert,
  Box,
  Button,
  Chip,
  CircularProgress,
  Divider,
  MenuItem,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from "@mui/material";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  type Dispatch,
  type ReactNode,
  type SetStateAction,
  useEffect,
  useMemo,
  useState,
} from "react";
import { api } from "../api/client";
import { formatUiDateTime } from "../lib/dateFormat";

type JsonRecord = Record<string, unknown>;

type SenderVerificationPanelProps = {
  autoRefresh: boolean;
};

type ChannelForm = {
  policy: string;
  allowed: string;
};

type ChannelKey = "slack" | "teams" | "whatsapp";

const EMPTY_CHANNEL_FORM: ChannelForm = {
  policy: "open",
  allowed: "",
};

function asRecord(value: unknown): JsonRecord {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as JsonRecord)
    : {};
}

function asRecords(value: unknown): JsonRecord[] {
  return Array.isArray(value) ? value.map(asRecord) : [];
}

function str(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function toBool(value: unknown): boolean {
  return value === true || value === "true" || value === 1;
}

function toStrings(value: unknown): string[] {
  return Array.isArray(value)
    ? value
        .map((item) => (typeof item === "string" ? item.trim() : ""))
        .filter(Boolean)
    : [];
}

function errMessage(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === "string") return error;
  const record = asRecord(error);
  return str(record.error, str(record.message, "Request failed"));
}

function humanTs(value: string): string {
  return formatUiDateTime(value, { fallback: "-" });
}

function csv(values: string[]): string {
  return values.join(", ");
}

function parseCsv(value: string): string[] {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

function channelLabel(channel: string): string {
  switch (channel) {
    case "slack":
      return "Slack";
    case "teams":
      return "Teams";
    case "whatsapp":
      return "WhatsApp";
    default:
      return channel || "-";
  }
}

function senderDisplay(row: JsonRecord): string {
  return str(row.sender_label, str(row.sender_id, "-"));
}

function scopeDisplay(row: JsonRecord): string {
  return str(row.scope_label, str(row.scope_id, "-"));
}

function rowKey(row: JsonRecord, index: number): string {
  return (
    str(row.key) ||
    [
      str(row.channel),
      str(row.sender_id),
      str(row.scope_id),
      str(row.conversation_id),
      String(index),
    ].join(":")
  );
}

function policyLabel(policy: string): string {
  return policy === "pairing" ? "Pairing required" : "Open";
}

function policyDetail(policy: string): string {
  return policy === "pairing"
    ? "Unknown senders wait for operator approval."
    : "Authenticated channel senders can trigger work.";
}

function trustCount(form: ChannelForm): number {
  return parseCsv(form.allowed).length;
}

function SummaryTile({
  label,
  value,
  detail,
  icon,
}: {
  label: string;
  value: string;
  detail: string;
  icon: ReactNode;
}) {
  return (
    <Box
      sx={{
        minWidth: 0,
        p: 1.35,
        borderRadius: 2,
        border: "1px solid var(--ui-rgba-255-255-255-080)",
        background: "var(--ui-rgba-255-255-255-030)",
      }}
    >
      <Stack direction="row" spacing={1} sx={{ alignItems: "flex-start" }}>
        <Box
          sx={{
            width: 32,
            height: 32,
            borderRadius: 1.5,
            display: "grid",
            placeItems: "center",
            color: "var(--ui-rgba-57-208-255-850)",
            background: "var(--ui-rgba-57-208-255-080)",
            flexShrink: 0,
          }}
        >
          {icon}
        </Box>
        <Stack spacing={0.2} sx={{ minWidth: 0 }}>
          <Typography
            variant="caption"
            sx={{ color: "text.secondary", lineHeight: 1.2 }}
          >
            {label}
          </Typography>
          <Typography variant="h6" sx={{ lineHeight: 1.1 }}>
            {value}
          </Typography>
          <Typography
            variant="caption"
            sx={{ color: "text.secondary", lineHeight: 1.35 }}
          >
            {detail}
          </Typography>
        </Stack>
      </Stack>
    </Box>
  );
}

function EmptyState({
  icon,
  title,
  body,
}: {
  icon: ReactNode;
  title: string;
  body: string;
}) {
  return (
    <Box
      sx={{
        border: "1px dashed var(--ui-rgba-255-255-255-120)",
        borderRadius: 2,
        p: 2,
        background: "var(--ui-rgba-255-255-255-020)",
      }}
    >
      <Stack direction="row" spacing={1.2} sx={{ alignItems: "flex-start" }}>
        <Box
          sx={{
            width: 34,
            height: 34,
            borderRadius: 1.5,
            display: "grid",
            placeItems: "center",
            color: "var(--ui-rgba-255-220-145-900)",
            background: "var(--ui-rgba-255-220-145-080)",
            flexShrink: 0,
          }}
        >
          {icon}
        </Box>
        <Stack spacing={0.25}>
          <Typography variant="body2" sx={{ fontWeight: 700 }}>
            {title}
          </Typography>
          <Typography
            variant="body2"
            sx={{ color: "text.secondary", maxWidth: 620 }}
          >
            {body}
          </Typography>
        </Stack>
      </Stack>
    </Box>
  );
}

export function SenderVerificationPanel({
  autoRefresh,
}: SenderVerificationPanelProps) {
  const queryClient = useQueryClient();
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const [slack, setSlack] = useState<ChannelForm>(EMPTY_CHANNEL_FORM);
  const [teams, setTeams] = useState<ChannelForm>(EMPTY_CHANNEL_FORM);
  const [whatsapp, setWhatsapp] = useState<ChannelForm>({
    policy: "pairing",
    allowed: "",
  });

  const overviewQ = useQuery({
    queryKey: ["settings-sender-verification"],
    queryFn: () => api.rawGet("/sender-verification"),
    refetchInterval: autoRefresh ? 8000 : false,
  });

  const saveSettings = useMutation({
    mutationFn: (payload: JsonRecord) =>
      api.rawPost("/sender-verification/settings", payload),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["settings-sender-verification"],
      });
    },
  });
  const approveSender = useMutation({
    mutationFn: (payload: JsonRecord) =>
      api.rawPost("/sender-verification/approve", payload),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["settings-sender-verification"],
      });
      await queryClient.invalidateQueries({ queryKey: ["notifications"] });
      await queryClient.invalidateQueries({ queryKey: ["notifications-count"] });
    },
  });
  const revokeSender = useMutation({
    mutationFn: (payload: JsonRecord) =>
      api.rawPost("/sender-verification/revoke", payload),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: ["settings-sender-verification"],
      });
      await queryClient.invalidateQueries({ queryKey: ["notifications"] });
      await queryClient.invalidateQueries({ queryKey: ["notifications-count"] });
    },
  });

  const payload = asRecord(overviewQ.data);
  const settings = asRecord(payload.settings);
  const slackSettings = asRecord(settings.slack);
  const teamsSettings = asRecord(settings.teams);
  const whatsappSettings = asRecord(settings.whatsapp);
  const pending = useMemo(() => asRecords(payload.pending), [payload]);
  const approved = useMemo(() => asRecords(payload.approved), [payload]);

  useEffect(() => {
    if (dirty) return;
    setSlack({
      policy: str(slackSettings.policy, "open"),
      allowed: csv(toStrings(slackSettings.allowed_senders)),
    });
    setTeams({
      policy: str(teamsSettings.policy, "open"),
      allowed: csv(toStrings(teamsSettings.allowed_senders)),
    });
    setWhatsapp({
      policy: str(whatsappSettings.policy, "pairing"),
      allowed: csv(toStrings(whatsappSettings.allowed_senders)),
    });
  }, [dirty, slackSettings, teamsSettings, whatsappSettings]);

  function updateChannel(
    setter: Dispatch<SetStateAction<ChannelForm>>,
    field: keyof ChannelForm,
    value: string,
  ) {
    setDirty(true);
    setter((current) => ({ ...current, [field]: value }));
  }

  async function handleSave() {
    setError(null);
    setSuccess(null);
    try {
      const payload: JsonRecord = {
        slack_policy: slack.policy,
        slack_allowed_senders: parseCsv(slack.allowed),
        teams_policy: teams.policy,
        teams_allowed_senders: parseCsv(teams.allowed),
      };
      if (toBool(whatsappSettings.configured)) {
        payload.whatsapp_policy = whatsapp.policy;
        payload.whatsapp_allowed_senders = parseCsv(whatsapp.allowed);
      }
      await saveSettings.mutateAsync(payload);
      setDirty(false);
      setSuccess("Sender trust policies saved.");
    } catch (e) {
      setError(errMessage(e));
    }
  }

  async function handleApprove(row: JsonRecord) {
    setError(null);
    setSuccess(null);
    try {
      await approveSender.mutateAsync({
        channel: str(row.channel),
        sender_id: str(row.sender_id),
        sender_label: str(row.sender_label) || undefined,
        scope_id: str(row.scope_id) || undefined,
        scope_label: str(row.scope_label) || undefined,
        conversation_id: str(row.conversation_id) || undefined,
        approved_by: "settings_ui",
      });
      setSuccess(`Approved ${senderDisplay(row)}.`);
    } catch (e) {
      setError(errMessage(e));
    }
  }

  async function handleRevoke(row: JsonRecord) {
    setError(null);
    setSuccess(null);
    try {
      await revokeSender.mutateAsync({
        channel: str(row.channel),
        sender_id: str(row.sender_id),
        scope_id: str(row.scope_id) || undefined,
      });
      setSuccess(`Revoked ${senderDisplay(row)}.`);
    } catch (e) {
      setError(errMessage(e));
    }
  }

  const initialLoading = overviewQ.isLoading && !overviewQ.data;
  const mutating =
    saveSettings.isPending || approveSender.isPending || revokeSender.isPending;
  const refreshing = overviewQ.isFetching && !initialLoading;

  const channels: Array<{
    key: ChannelKey;
    title: string;
    form: ChannelForm;
    setForm: Dispatch<SetStateAction<ChannelForm>>;
    configured: boolean;
    description: string;
    helper: string;
  }> = [
    {
      key: "slack",
      title: "Slack",
      form: slack,
      setForm: setSlack,
      configured: toBool(slackSettings.configured),
      description: "Workspace messages and app mentions.",
      helper: "Comma-separated Slack user IDs.",
    },
    {
      key: "teams",
      title: "Teams",
      form: teams,
      setForm: setTeams,
      configured: toBool(teamsSettings.configured),
      description: "Teams conversations and channel posts.",
      helper: "Comma-separated Teams or AAD sender IDs.",
    },
    {
      key: "whatsapp",
      title: "WhatsApp",
      form: whatsapp,
      setForm: setWhatsapp,
      configured: toBool(whatsappSettings.configured),
      description: "Bridge messages from phone numbers.",
      helper: "Comma-separated trusted phone numbers.",
    },
  ];

  const configuredCount = channels.filter((channel) => channel.configured).length;
  const pairedCount = channels.filter(
    (channel) => channel.form.policy === "pairing" && channel.configured,
  ).length;
  const trustedStaticCount = channels.reduce(
    (count, channel) => count + trustCount(channel.form),
    0,
  );

  return (
    <Stack spacing={2.25}>
      <Box
        className="list-shell"
        sx={{
          p: 2,
          borderColor: "var(--ui-rgba-57-208-255-120) !important",
          background:
            "linear-gradient(135deg, var(--ui-rgba-57-208-255-040), var(--ui-rgba-255-255-255-020)) !important",
        }}
      >
        <Stack
          direction={{ xs: "column", md: "row" }}
          spacing={1.5}
          sx={{ justifyContent: "space-between", alignItems: "flex-start" }}
        >
          <Stack direction="row" spacing={1.3} sx={{ minWidth: 0 }}>
            <Box
              sx={{
                width: 40,
                height: 40,
                borderRadius: 2,
                display: "grid",
                placeItems: "center",
                color: "var(--ui-rgba-57-208-255-850)",
                background: "var(--ui-rgba-57-208-255-100)",
                flexShrink: 0,
              }}
            >
              <ShieldOutlinedIcon fontSize="small" />
            </Box>
            <Stack spacing={0.4} sx={{ minWidth: 0 }}>
              <Stack
                direction="row"
                spacing={0.8}
                useFlexGap
                sx={{ alignItems: "center", flexWrap: "wrap" }}
              >
                <Typography variant="h6" sx={{ lineHeight: 1.15 }}>
                  Sender verification control
                </Typography>
                <Chip
                  size="small"
                  color={dirty ? "warning" : "success"}
                  variant={dirty ? "outlined" : "filled"}
                  label={dirty ? "Unsaved changes" : "Saved"}
                />
                {refreshing ? (
                  <Chip size="small" variant="outlined" label="Refreshing" />
                ) : null}
              </Stack>
              <Typography
                variant="body2"
                sx={{ color: "text.secondary", maxWidth: 760 }}
              >
                Transport signatures prove the request came from Slack, Teams,
                or WhatsApp. Sender trust decides which authenticated humans can
                start AgentArk work.
              </Typography>
            </Stack>
          </Stack>
          <Button
            variant="contained"
            startIcon={
              saveSettings.isPending ? (
                <CircularProgress color="inherit" size={15} />
              ) : (
                <SaveRoundedIcon fontSize="small" />
              )
            }
            onClick={handleSave}
            disabled={!dirty || saveSettings.isPending || initialLoading}
            sx={{ minWidth: 136 }}
          >
            {saveSettings.isPending ? "Saving" : "Save Policies"}
          </Button>
        </Stack>

        <Box
          sx={{
            display: "grid",
            gridTemplateColumns: {
              xs: "1fr",
              sm: "repeat(2, minmax(0, 1fr))",
              lg: "repeat(4, minmax(0, 1fr))",
            },
            gap: 1,
            mt: 1.6,
          }}
        >
          <SummaryTile
            icon={<HourglassTopRoundedIcon fontSize="small" />}
            label="Pending"
            value={String(pending.length)}
            detail="Needs approval before work runs"
          />
          <SummaryTile
            icon={<VerifiedUserRoundedIcon fontSize="small" />}
            label="Approved"
            value={String(approved.length)}
            detail="Trusted for paired channels"
          />
          <SummaryTile
            icon={<CheckCircleRoundedIcon fontSize="small" />}
            label="Configured"
            value={`${configuredCount} / ${channels.length}`}
            detail="Channels with transport setup"
          />
          <SummaryTile
            icon={<DoneAllRoundedIcon fontSize="small" />}
            label="Always trusted"
            value={String(trustedStaticCount)}
            detail="Static IDs across policies"
          />
        </Box>
      </Box>

      {overviewQ.error ? (
        <Alert severity="error">{errMessage(overviewQ.error)}</Alert>
      ) : null}
      {error ? <Alert severity="error">{error}</Alert> : null}
      {success ? <Alert severity="success">{success}</Alert> : null}
      {initialLoading ? (
        <Alert severity="info">Loading sender verification state...</Alert>
      ) : null}

      <Box className="list-shell" sx={{ p: 2 }}>
        <Stack spacing={1.5}>
          <Stack
            direction={{ xs: "column", md: "row" }}
            spacing={1}
            sx={{ justifyContent: "space-between", alignItems: "flex-start" }}
          >
            <Box>
              <Typography variant="h6" sx={{ lineHeight: 1.2 }}>
                Trust policies
              </Typography>
              <Typography variant="body2" sx={{ color: "text.secondary" }}>
                Use pairing for channels where a new human sender should pause
                before AgentArk takes action.
              </Typography>
            </Box>
          </Stack>

          <Box
            sx={{
              display: "grid",
              gridTemplateColumns: {
                xs: "1fr",
                lg: "repeat(3, minmax(0, 1fr))",
              },
              gap: 1.25,
            }}
          >
            {channels.map((channel) => {
              const disabled = channel.key === "whatsapp" && !channel.configured;
              return (
                <Box
                  key={channel.key}
                  sx={{
                    p: 1.45,
                    borderRadius: 2,
                    border: "1px solid",
                    borderColor:
                      channel.form.policy === "pairing"
                        ? "var(--ui-rgba-255-220-145-220)"
                        : "var(--ui-rgba-255-255-255-080)",
                    background:
                      channel.form.policy === "pairing"
                        ? "var(--ui-rgba-255-220-145-050)"
                        : "var(--ui-rgba-255-255-255-025)",
                    minWidth: 0,
                  }}
                >
                  <Stack spacing={1.25}>
                    <Stack
                      direction="row"
                      spacing={1}
                      sx={{ justifyContent: "space-between", gap: 1 }}
                    >
                      <Box sx={{ minWidth: 0 }}>
                        <Typography variant="subtitle2" sx={{ fontWeight: 800 }}>
                          {channel.title}
                        </Typography>
                        <Typography
                          variant="caption"
                          sx={{ color: "text.secondary", display: "block" }}
                        >
                          {channel.description}
                        </Typography>
                      </Box>
                      <Chip
                        size="small"
                        color={channel.configured ? "success" : "default"}
                        variant={channel.configured ? "filled" : "outlined"}
                        label={channel.configured ? "Configured" : "Not configured"}
                      />
                    </Stack>

                    <Divider />

                    <Stack spacing={1.2}>
                      <TextField
                        select
                        label="Policy"
                        size="small"
                        value={channel.form.policy}
                        onChange={(event) =>
                          updateChannel(
                            channel.setForm,
                            "policy",
                            event.target.value,
                          )
                        }
                        disabled={disabled}
                        fullWidth
                        helperText={policyDetail(channel.form.policy)}
                      >
                        <MenuItem value="open">Open</MenuItem>
                        <MenuItem value="pairing">Pairing</MenuItem>
                      </TextField>
                      <TextField
                        label="Trusted sender IDs"
                        size="small"
                        value={channel.form.allowed}
                        onChange={(event) =>
                          updateChannel(
                            channel.setForm,
                            "allowed",
                            event.target.value,
                          )
                        }
                        multiline
                        minRows={2}
                        fullWidth
                        disabled={disabled}
                        helperText={channel.helper}
                      />
                    </Stack>

                    <Stack
                      direction="row"
                      spacing={0.75}
                      useFlexGap
                      sx={{ alignItems: "center", flexWrap: "wrap" }}
                    >
                      <Chip
                        size="small"
                        variant="outlined"
                        label={policyLabel(channel.form.policy)}
                      />
                      <Chip
                        size="small"
                        variant="outlined"
                        label={`${trustCount(channel.form)} trusted ID${
                          trustCount(channel.form) === 1 ? "" : "s"
                        }`}
                      />
                    </Stack>
                  </Stack>
                </Box>
              );
            })}
          </Box>
        </Stack>
      </Box>

      <Box className="list-shell" sx={{ p: 2 }}>
        <Stack spacing={1.4}>
          <Stack
            direction="row"
            sx={{ justifyContent: "space-between", alignItems: "flex-start" }}
          >
            <Box>
              <Typography variant="h6" sx={{ lineHeight: 1.2 }}>
                Pending approvals
              </Typography>
              <Typography variant="body2" sx={{ color: "text.secondary" }}>
                New senders from paired channels appear here before work starts.
              </Typography>
            </Box>
            <Chip size="small" label={`${pending.length} pending`} />
          </Stack>

          {pending.length === 0 ? (
            <EmptyState
              icon={<HourglassTopRoundedIcon fontSize="small" />}
              title="No senders are waiting"
              body="Pairing is active only when a configured channel receives a message from an untrusted sender."
            />
          ) : (
            <TableContainer
              sx={{
                border: "1px solid var(--ui-rgba-255-255-255-080)",
                borderRadius: 2,
              }}
            >
              <Table size="small">
                <TableHead>
                  <TableRow>
                    <TableCell>Channel</TableCell>
                    <TableCell>Sender</TableCell>
                    <TableCell>Scope</TableCell>
                    <TableCell>Seen</TableCell>
                    <TableCell>Attempts</TableCell>
                    <TableCell>Preview</TableCell>
                    <TableCell align="right">Action</TableCell>
                  </TableRow>
                </TableHead>
                <TableBody>
                  {pending.map((row, index) => (
                    <TableRow key={rowKey(row, index)}>
                      <TableCell>{channelLabel(str(row.channel))}</TableCell>
                      <TableCell>
                        <Stack spacing={0.2}>
                          <Typography variant="body2" sx={{ fontWeight: 700 }}>
                            {senderDisplay(row)}
                          </Typography>
                          <Typography
                            variant="caption"
                            sx={{ color: "text.secondary" }}
                          >
                            {str(row.sender_id, "-")}
                          </Typography>
                        </Stack>
                      </TableCell>
                      <TableCell>{scopeDisplay(row)}</TableCell>
                      <TableCell>
                        <Stack spacing={0.2}>
                          <Typography variant="body2">
                            {humanTs(str(row.last_seen_at))}
                          </Typography>
                          <Typography
                            variant="caption"
                            sx={{ color: "text.secondary" }}
                          >
                            First {humanTs(str(row.first_seen_at))}
                          </Typography>
                        </Stack>
                      </TableCell>
                      <TableCell>{String(row.occurrences ?? 1)}</TableCell>
                      <TableCell>
                        <Typography
                          variant="body2"
                          sx={{
                            maxWidth: 320,
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                          }}
                          title={str(row.message_preview, "-")}
                        >
                          {str(row.message_preview, "-")}
                        </Typography>
                      </TableCell>
                      <TableCell align="right">
                        <Button
                          size="small"
                          variant="contained"
                          startIcon={<CheckCircleRoundedIcon fontSize="small" />}
                          onClick={() => handleApprove(row)}
                          disabled={mutating}
                        >
                          Approve
                        </Button>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </TableContainer>
          )}
        </Stack>
      </Box>

      <Box className="list-shell" sx={{ p: 2 }}>
        <Stack spacing={1.4}>
          <Stack
            direction="row"
            sx={{ justifyContent: "space-between", alignItems: "flex-start" }}
          >
            <Box>
              <Typography variant="h6" sx={{ lineHeight: 1.2 }}>
                Approved senders
              </Typography>
              <Typography variant="body2" sx={{ color: "text.secondary" }}>
                These senders can start AgentArk work on paired channels until
                revoked.
              </Typography>
            </Box>
            <Chip size="small" label={`${approved.length} approved`} />
          </Stack>

          {approved.length === 0 ? (
            <EmptyState
              icon={<VerifiedUserRoundedIcon fontSize="small" />}
              title="No persistent approvals yet"
              body="Approving a pending sender creates a scoped trust record that can be revoked later."
            />
          ) : (
            <TableContainer
              sx={{
                border: "1px solid var(--ui-rgba-255-255-255-080)",
                borderRadius: 2,
              }}
            >
              <Table size="small">
                <TableHead>
                  <TableRow>
                    <TableCell>Channel</TableCell>
                    <TableCell>Sender</TableCell>
                    <TableCell>Scope</TableCell>
                    <TableCell>Approved</TableCell>
                    <TableCell>By</TableCell>
                    <TableCell align="right">Action</TableCell>
                  </TableRow>
                </TableHead>
                <TableBody>
                  {approved.map((row, index) => (
                    <TableRow key={rowKey(row, index)}>
                      <TableCell>{channelLabel(str(row.channel))}</TableCell>
                      <TableCell>
                        <Stack spacing={0.2}>
                          <Typography variant="body2" sx={{ fontWeight: 700 }}>
                            {senderDisplay(row)}
                          </Typography>
                          <Typography
                            variant="caption"
                            sx={{ color: "text.secondary" }}
                          >
                            {str(row.sender_id, "-")}
                          </Typography>
                        </Stack>
                      </TableCell>
                      <TableCell>{scopeDisplay(row)}</TableCell>
                      <TableCell>{humanTs(str(row.approved_at))}</TableCell>
                      <TableCell>{str(row.approved_by, "-")}</TableCell>
                      <TableCell align="right">
                        <Button
                          size="small"
                          color="warning"
                          variant="outlined"
                          onClick={() => handleRevoke(row)}
                          disabled={mutating}
                        >
                          Revoke
                        </Button>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </TableContainer>
          )}
        </Stack>
      </Box>
    </Stack>
  );
}
