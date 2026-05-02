//! Orbit-management tools surfaced to the agent loop.
//!
//! Slice 3 introduces the first orbit-management tool: `arkorbit_create_orbit`.
//! Renames, deletions, and per-orbit settings remain user-driven from the UI;
//! the agent should not delete a user's canvas on its own.

use anyhow::Result;
use serde_json::Value;

use crate::core::arkorbit::ArkOrbitService;

use super::validators::{optional_string, require_string};

/// Create a fresh orbit owned by the active user. Mirrors to disk under
/// `<data_dir>/arkorbit/<orbit_id>/orbit.json` via the service layer.
pub async fn create_orbit(
    service: &ArkOrbitService,
    user_id: &str,
    args: &Value,
) -> Result<String> {
    let name = require_string(args, "name")?.to_string();
    let icon = optional_string(args, "icon").map(|s| s.to_string());
    let color = optional_string(args, "color").map(|s| s.to_string());
    let agent_instructions = optional_string(args, "agent_instructions").map(|s| s.to_string());

    let orbit = service
        .create_orbit(user_id, &name, icon, color, agent_instructions)
        .await?;
    Ok(serde_json::to_string(
        &serde_json::json!({ "orbit": orbit }),
    )?)
}
