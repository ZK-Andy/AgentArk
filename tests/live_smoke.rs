pub const LIVE_SMOKE_ENV: &str = "AGENTARK_RUN_LIVE_SMOKES";

pub fn live_smoke_enabled() -> bool {
    matches!(
        std::env::var(LIVE_SMOKE_ENV)
            .ok()
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}
