pub const INLINE_CHART_FENCE_LANGUAGE: &str = "agentark-chart";

pub fn inline_visualization_guidance() -> &'static str {
    "Visual analysis inside the current conversation is an inline answer capability, not app delivery. When the intended outcome is a report, research synthesis, analysis, or answer in chat, use prose and tables for exact values, and include fenced `agentark-chart` JSON blocks when the user asks for visuals or when a chart materially clarifies quantitative comparisons, trends, distributions, proportions, uncertainty, evidence coverage, or grouped breakdowns. Choose line or area charts for ordered/time-series trends, bar charts for categorical comparisons, scatter charts for relationships, and pie/doughnut charts only for compact part-whole breakdowns. Use app delivery only when the intended final object is a managed browser-runnable, reusable, hosted, or previewable experience. The chart block schema is: {\"title\":\"...\",\"type\":\"bar|line|area|scatter|pie|doughnut\",\"x\":\"category_field\",\"series\":[{\"key\":\"numeric_field\",\"name\":\"label\"}],\"data\":[{\"category_field\":\"A\",\"numeric_field\":1}]}."
}

pub fn app_delivery_boundary_guidance() -> &'static str {
    "Use deployment only when the intended result is a managed browser-usable, runnable, hosted, previewable, or interactive experience. Do not infer deployment merely because an immediate answer, report, research synthesis, or analysis should be visually structured; conversation-native reports remain current-answer work unless the desired final object is a managed experience."
}

pub fn app_deploy_inline_report_boundary() -> &'static str {
    "Do not use app_deploy for immediate chat reports, research syntheses, or analyses that merely need visual summaries; those should remain in the conversation response with inline tables/charts when useful."
}

pub fn inline_chart_block(chart: &serde_json::Value) -> String {
    let body = serde_json::to_string_pretty(chart).unwrap_or_else(|_| "{}".to_string());
    format!("```{}\n{}\n```", INLINE_CHART_FENCE_LANGUAGE, body)
}
