// OrbitSettingsDialog - rename, set icon/color, edit per-orbit agent
// instructions, or delete an orbit.
//
// The delete branch is treated as destructive: we surface a second
// confirmation that explicitly names the cascade (every orbit file
// orbit goes with it). This is in addition to the structural delete on
// the backend; double confirmation is for user trust, not validation.

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import { arkorbitApi } from "./api";
import type { Orbit, OrbitId, OrbitPatch } from "./types";

type Props = {
  orbitId: OrbitId | null;
  orbits: Orbit[];
  open: boolean;
  onClose: () => void;
  onUpdated: (orbit: Orbit) => void;
  onDeleted: (id: OrbitId) => void;
};

function findOrbit(orbits: Orbit[], id: OrbitId | null): Orbit | null {
  if (!id) return null;
  return orbits.find((o) => o.id === id) ?? null;
}

export function OrbitSettingsDialog({
  orbitId,
  orbits,
  open,
  onClose,
  onUpdated,
  onDeleted,
}: Props) {
  const orbit = useMemo(() => findOrbit(orbits, orbitId), [orbits, orbitId]);

  const [name, setName] = useState("");
  const [icon, setIcon] = useState("");
  const [color, setColor] = useState("");
  const [agentInstructions, setAgentInstructions] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open || !orbit) return;
    setName(orbit.name);
    setIcon(orbit.icon ?? "");
    setColor(orbit.color ?? "");
    setAgentInstructions(orbit.agent_instructions ?? "");
    setConfirmDelete(false);
    setError(null);
  }, [open, orbit]);

  const handleSave = useCallback(async () => {
    if (!orbit) return;
    const trimmedName = name.trim();
    if (!trimmedName) {
      setError("Name is required.");
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      const patch: OrbitPatch = {};
      if (trimmedName !== orbit.name) patch.name = trimmedName;
      // Distinguish "clear" (empty after trim) from "leave alone" (unchanged).
      const desiredIcon = icon.trim() || null;
      const desiredColor = color.trim() || null;
      const desiredInstructions = agentInstructions.trim() || null;
      if (desiredIcon !== (orbit.icon ?? null)) patch.icon = desiredIcon;
      if (desiredColor !== (orbit.color ?? null)) patch.color = desiredColor;
      if (desiredInstructions !== (orbit.agent_instructions ?? null)) {
        patch.agent_instructions = desiredInstructions;
      }
      if (Object.keys(patch).length === 0) {
        onClose();
        return;
      }
      const updated = await arkorbitApi.updateOrbit(orbit.id, patch);
      if (!updated) {
        setError("Server did not return the updated orbit.");
        return;
      }
      onUpdated(updated);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  }, [orbit, name, icon, color, agentInstructions, onUpdated, onClose]);

  const handleDelete = useCallback(async () => {
    if (!orbit) return;
    setSubmitting(true);
    setError(null);
    try {
      await arkorbitApi.deleteOrbit(orbit.id);
      onDeleted(orbit.id);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  }, [orbit, onDeleted, onClose]);

  if (!orbit) return null;

  return (
    <Dialog
      open={open}
      onClose={() => (submitting ? undefined : onClose())}
      maxWidth="sm"
      fullWidth
    >
      <DialogTitle>Orbit settings</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ pt: 1 }}>
          <TextField
            label="Name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            size="small"
            fullWidth
          />
          <Stack direction="row" spacing={2}>
            <TextField
              label="Icon"
              value={icon}
              onChange={(e) => setIcon(e.target.value)}
              size="small"
              sx={{ flex: 1 }}
              slotProps={{ htmlInput: { maxLength: 8 } }}
              placeholder="Optional"
            />
            <TextField
              label="Color"
              value={color}
              onChange={(e) => setColor(e.target.value)}
              size="small"
              sx={{ flex: 1 }}
              placeholder="#7c3aed"
            />
          </Stack>
          <Box>
            <Typography variant="caption" color="text.secondary">
              Per-orbit instructions
            </Typography>
            <TextField
              value={agentInstructions}
              onChange={(e) => setAgentInstructions(e.target.value)}
              size="small"
              fullWidth
              multiline
              minRows={3}
              maxRows={10}
              placeholder="Anything the agent should know whenever you chat inside this canvas. Forwarded as context only - never an override of your configured chat model."
            />
          </Box>
          {confirmDelete ? (
            <Alert severity="error">
              This will permanently delete{" "}
              <strong>{orbit.name}</strong> and every file in it. This
              cannot be undone.
            </Alert>
          ) : null}
          {error ? (
            <Typography variant="caption" color="error">
              {error}
            </Typography>
          ) : null}
        </Stack>
      </DialogContent>
      <DialogActions sx={{ justifyContent: "space-between", px: 3, pb: 2 }}>
        {confirmDelete ? (
          <Stack direction="row" spacing={1}>
            <Button
              size="small"
              onClick={() => setConfirmDelete(false)}
              disabled={submitting}
            >
              Cancel delete
            </Button>
            <Button
              size="small"
              color="error"
              variant="contained"
              onClick={handleDelete}
              disabled={submitting}
            >
              Yes, delete this orbit
            </Button>
          </Stack>
        ) : (
          <Button
            size="small"
            color="error"
            onClick={() => setConfirmDelete(true)}
            disabled={submitting}
          >
            Delete orbit...
          </Button>
        )}
        <Stack direction="row" spacing={1}>
          <Button onClick={onClose} disabled={submitting} size="small">
            Cancel
          </Button>
          <Button
            onClick={handleSave}
            variant="contained"
            size="small"
            disabled={submitting || !name.trim()}
          >
            Save
          </Button>
        </Stack>
      </DialogActions>
    </Dialog>
  );
}

export default OrbitSettingsDialog;
