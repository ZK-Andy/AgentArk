//! MCP registry for managing external servers and tool/resource bindings

use anyhow::{anyhow, Result};
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

use crate::actions::ActionDef;
use crate::core::config::{
    AgentConfig, McpAuthConfig, McpAuthSecret, McpServerConfig, McpTransportConfig, Secrets,
};
use crate::runtime::{ActionRuntime, McpBinding, McpBindingKind};
use crate::safety::{RuleAction, RuleTrigger, SafetyEngine, SafetyRule};

use super::client::{McpAuth, McpClient};
use super::{McpResource, McpTool};

#[derive(Debug, Clone, Serialize)]
pub struct McpServerView {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub resources_enabled: bool,
    pub transport: McpTransportView,
    pub auth: McpAuthView,
    pub tool_allowlist: Vec<String>,
    pub resource_allowlist: Vec<String>,
    pub timeout_secs: u64,
    pub max_response_bytes: usize,
    pub tool_count: usize,
    pub resource_count: usize,
    pub warnings: Vec<String>,
    pub last_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<McpTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<Vec<McpResource>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpTransportView {
    Http {
        url: String,
    },
    Stdio {
        command: String,
        args: Vec<String>,
        working_dir: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct McpAuthView {
    pub auth_type: String,
    pub has_auth: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

pub struct McpRegistry {
    servers: HashMap<String, McpServerState>,
}

struct McpServerState {
    config: McpServerConfig,
    client: tokio::sync::Mutex<McpClient>,
    tools: Vec<McpTool>,
    resources: Vec<McpResource>,
    warnings: Vec<String>,
    last_error: Option<String>,
    has_auth: bool,
}

impl McpRegistry {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
        }
    }

    pub fn list_servers(&self, include_details: bool) -> Vec<McpServerView> {
        self.servers
            .values()
            .map(|s| s.view(include_details))
            .collect()
    }

    pub fn get_server(&self, id: &str, include_details: bool) -> Option<McpServerView> {
        self.servers.get(id).map(|s| s.view(include_details))
    }

    pub async fn sync_from_config(
        &mut self,
        config: &AgentConfig,
        secrets: &Secrets,
        runtime: &ActionRuntime,
        safety: &mut SafetyEngine,
    ) -> Result<()> {
        runtime.unregister_mcp_actions().await;
        self.servers.clear();

        for server in &config.mcp.servers {
            let auth_secret = secrets.mcp_auth.get(&server.id);
            let (auth, auth_warnings, has_auth) = resolve_auth(&server.auth, auth_secret);
            let mut warnings = compute_mcp_warnings(server);
            warnings.extend(auth_warnings);

            let mut client = McpClient::new(server, auth)?;
            let mut tools = Vec::new();
            let mut resources = Vec::new();
            let mut last_error = None;

            if server.enabled {
                match client.list_tools().await {
                    Ok(list) => tools = filter_tools(list, &server.tool_allowlist),
                    Err(e) => last_error = Some(e.to_string()),
                }

                if server.resources_enabled {
                    match client.list_resources().await {
                        Ok(list) => resources = filter_resources(list, &server.resource_allowlist),
                        Err(e) => last_error = Some(e.to_string()),
                    }
                }
            }

            if server.enabled && last_error.is_none() {
                register_actions(runtime, safety, server, &tools, &resources).await?;
            }

            let state = McpServerState {
                config: server.clone(),
                client: tokio::sync::Mutex::new(client),
                tools,
                resources,
                warnings,
                last_error,
                has_auth,
            };
            self.servers.insert(server.id.clone(), state);
        }

        Ok(())
    }

    pub async fn refresh_server(
        &mut self,
        id: &str,
        runtime: &ActionRuntime,
        safety: &mut SafetyEngine,
    ) -> Result<()> {
        let Some(state) = self.servers.get_mut(id) else {
            return Err(anyhow!("MCP server not found"));
        };

        runtime.unregister_mcp_actions_for_server(id).await;

        let mut client = state.client.lock().await;
        state.last_error = None;
        let mut tools = Vec::new();
        let mut resources = Vec::new();
        if state.config.enabled {
            match client.list_tools().await {
                Ok(list) => tools = filter_tools(list, &state.config.tool_allowlist),
                Err(e) => state.last_error = Some(e.to_string()),
            }

            if state.config.resources_enabled {
                match client.list_resources().await {
                    Ok(list) => {
                        resources = filter_resources(list, &state.config.resource_allowlist)
                    }
                    Err(e) => state.last_error = Some(e.to_string()),
                }
            }
        }

        state.tools = tools;
        state.resources = resources;
        if state.config.enabled && state.last_error.is_none() {
            register_actions(
                runtime,
                safety,
                &state.config,
                &state.tools,
                &state.resources,
            )
            .await?;
        }

        Ok(())
    }

    pub async fn call_tool(
        &mut self,
        server_id: &str,
        tool_name: &str,
        arguments: &Value,
    ) -> Result<String> {
        let state = self
            .servers
            .get_mut(server_id)
            .ok_or_else(|| anyhow!("MCP server not found"))?;
        if !state.config.enabled {
            return Err(anyhow!("MCP server is disabled"));
        }
        let mut client = state.client.lock().await;
        let result = client.call_tool(tool_name, arguments).await?;
        Ok(format_mcp_result(&result))
    }

    pub async fn read_resource(&mut self, server_id: &str, uri: &str) -> Result<String> {
        let state = self
            .servers
            .get_mut(server_id)
            .ok_or_else(|| anyhow!("MCP server not found"))?;
        if !state.config.enabled || !state.config.resources_enabled {
            return Err(anyhow!("MCP resources are disabled"));
        }
        let mut client = state.client.lock().await;
        let result = client.read_resource(uri).await?;
        Ok(format_mcp_result(&result))
    }
}

impl McpServerState {
    fn view(&self, include_details: bool) -> McpServerView {
        McpServerView {
            id: self.config.id.clone(),
            name: self.config.name.clone(),
            description: self.config.description.clone(),
            enabled: self.config.enabled,
            resources_enabled: self.config.resources_enabled,
            transport: transport_view(&self.config.transport),
            auth: auth_view(&self.config.auth, self.has_auth),
            tool_allowlist: self.config.tool_allowlist.clone(),
            resource_allowlist: self.config.resource_allowlist.clone(),
            timeout_secs: self.config.timeout_secs,
            max_response_bytes: self.config.max_response_bytes,
            tool_count: self.tools.len(),
            resource_count: self.resources.len(),
            warnings: self.warnings.clone(),
            last_error: self.last_error.clone(),
            tools: if include_details {
                Some(self.tools.clone())
            } else {
                None
            },
            resources: if include_details {
                Some(self.resources.clone())
            } else {
                None
            },
        }
    }
}

fn transport_view(transport: &McpTransportConfig) -> McpTransportView {
    match transport {
        McpTransportConfig::Http { url } => McpTransportView::Http { url: url.clone() },
        McpTransportConfig::Stdio {
            command,
            args,
            working_dir,
        } => McpTransportView::Stdio {
            command: command.clone(),
            args: args.clone(),
            working_dir: working_dir.clone(),
        },
    }
}

fn auth_view(auth: &Option<McpAuthConfig>, has_auth: bool) -> McpAuthView {
    match auth {
        None => McpAuthView {
            auth_type: "none".to_string(),
            has_auth,
            header: None,
            name: None,
        },
        Some(McpAuthConfig::Bearer { header }) => McpAuthView {
            auth_type: "bearer".to_string(),
            has_auth,
            header: Some(header.clone()),
            name: None,
        },
        Some(McpAuthConfig::Basic) => McpAuthView {
            auth_type: "basic".to_string(),
            has_auth,
            header: None,
            name: None,
        },
        Some(McpAuthConfig::Header { name }) => McpAuthView {
            auth_type: "header".to_string(),
            has_auth,
            header: None,
            name: Some(name.clone()),
        },
        Some(McpAuthConfig::Query { name }) => McpAuthView {
            auth_type: "query".to_string(),
            has_auth,
            header: None,
            name: Some(name.clone()),
        },
    }
}

pub fn compute_mcp_warnings(config: &McpServerConfig) -> Vec<String> {
    let mut warnings = Vec::new();
    match &config.transport {
        McpTransportConfig::Http { url } => {
            if let Ok(parsed) = url::Url::parse(url) {
                if parsed.scheme() != "https" {
                    warnings.push(
                        "MCP server uses non-TLS HTTP. Credentials and data may be exposed."
                            .to_string(),
                    );
                }
                if let Some(host) = parsed.host_str() {
                    if is_private_host(host) {
                        warnings.push("MCP server points to a private/local address. Only connect to servers you trust.".to_string());
                    }
                }
            } else {
                warnings.push("MCP server URL is invalid.".to_string());
            }
        }
        McpTransportConfig::Stdio { .. } => {
            warnings.push("MCP stdio runs a local process. Only use trusted binaries.".to_string());
        }
    }
    if config.resources_enabled {
        warnings.push("MCP resources are enabled. Resource content is untrusted and may contain malicious instructions.".to_string());
    }
    warnings
}

fn is_private_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return match ip {
            std::net::IpAddr::V4(v4) => v4.is_private() || v4.is_loopback() || v4.is_link_local(),
            std::net::IpAddr::V6(v6) => {
                v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local()
            }
        };
    }
    false
}

fn resolve_auth(
    auth: &Option<McpAuthConfig>,
    secret: Option<&McpAuthSecret>,
) -> (Option<McpAuth>, Vec<String>, bool) {
    let has_auth = secret
        .and_then(|s| {
            s.token
                .as_ref()
                .or(s.username.as_ref())
                .or(s.password.as_ref())
        })
        .is_some();
    let mut warnings = Vec::new();

    match auth {
        None => (None, warnings, has_auth),
        Some(McpAuthConfig::Bearer { header }) => {
            let token = secret.and_then(|s| s.token.clone()).unwrap_or_default();
            if token.is_empty() {
                warnings.push("MCP auth configured (bearer) but no token stored.".to_string());
                return (None, warnings, false);
            }
            (
                Some(McpAuth::Bearer {
                    header: header.clone(),
                    token,
                }),
                warnings,
                true,
            )
        }
        Some(McpAuthConfig::Basic) => {
            let username = secret.and_then(|s| s.username.clone()).unwrap_or_default();
            let password = secret.and_then(|s| s.password.clone()).unwrap_or_default();
            if username.is_empty() || password.is_empty() {
                warnings
                    .push("MCP auth configured (basic) but username/password missing.".to_string());
                return (None, warnings, false);
            }
            (Some(McpAuth::Basic { username, password }), warnings, true)
        }
        Some(McpAuthConfig::Header { name }) => {
            let value = secret.and_then(|s| s.token.clone()).unwrap_or_default();
            if value.is_empty() {
                warnings.push("MCP auth configured (header) but no value stored.".to_string());
                return (None, warnings, false);
            }
            (
                Some(McpAuth::Header {
                    name: name.clone(),
                    value,
                }),
                warnings,
                true,
            )
        }
        Some(McpAuthConfig::Query { name }) => {
            let value = secret.and_then(|s| s.token.clone()).unwrap_or_default();
            if value.is_empty() {
                warnings.push("MCP auth configured (query) but no value stored.".to_string());
                return (None, warnings, false);
            }
            (
                Some(McpAuth::Query {
                    name: name.clone(),
                    value,
                }),
                warnings,
                true,
            )
        }
    }
}

fn filter_tools(tools: Vec<McpTool>, allowlist: &[String]) -> Vec<McpTool> {
    if allowlist.is_empty() {
        return tools;
    }
    let allowed: HashSet<&str> = allowlist.iter().map(|s| s.as_str()).collect();
    tools
        .into_iter()
        .filter(|t| allowed.contains(t.name.as_str()))
        .collect()
}

fn filter_resources(resources: Vec<McpResource>, allowlist: &[String]) -> Vec<McpResource> {
    if allowlist.is_empty() {
        return resources;
    }
    let allowed: HashSet<&str> = allowlist.iter().map(|s| s.as_str()).collect();
    resources
        .into_iter()
        .filter(|r| allowed.contains(r.uri.as_str()))
        .collect()
}

async fn register_actions(
    runtime: &ActionRuntime,
    safety: &mut SafetyEngine,
    server: &McpServerConfig,
    tools: &[McpTool],
    resources: &[McpResource],
) -> Result<()> {
    let mut used_names: HashMap<String, usize> = HashMap::new();

    for tool in tools {
        let action_name = unique_action_name(&server.id, "tool", &tool.name, &mut used_names);
        let def = ActionDef {
            name: action_name.clone(),
            description: format!("MCP: {} - {}", server.name, tool.description),
            version: "1.0.0".to_string(),
            input_schema: tool.input_schema.clone(),
            capabilities: vec!["network".to_string()],
            sandbox_mode: Some(crate::runtime::SandboxMode::Native),
            source: crate::actions::ActionSource::System,
            file_path: None,
        };
        runtime
            .register_mcp_action(
                def,
                McpBinding {
                    server_id: server.id.clone(),
                    kind: McpBindingKind::Tool {
                        name: tool.name.clone(),
                    },
                },
            )
            .await;

        safety.add_rule(SafetyRule {
            name: format!("mcp_approve_{}", action_name),
            description: format!("MCP tool requires approval: {}", tool.name),
            trigger: RuleTrigger::Action { name: action_name },
            condition: None,
            action: RuleAction::RequireApproval,
            verified: true,
        });
    }

    for resource in resources {
        let action_name =
            unique_action_name(&server.id, "resource", &resource.name, &mut used_names);
        let def = ActionDef {
            name: action_name.clone(),
            description: format!("MCP resource: {} - {}", server.name, resource.description),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            capabilities: vec!["network".to_string()],
            sandbox_mode: Some(crate::runtime::SandboxMode::Native),
            source: crate::actions::ActionSource::System,
            file_path: None,
        };
        runtime
            .register_mcp_action(
                def,
                McpBinding {
                    server_id: server.id.clone(),
                    kind: McpBindingKind::Resource {
                        uri: resource.uri.clone(),
                    },
                },
            )
            .await;

        safety.add_rule(SafetyRule {
            name: format!("mcp_approve_{}", action_name),
            description: format!("MCP resource requires approval: {}", resource.name),
            trigger: RuleTrigger::Action { name: action_name },
            condition: None,
            action: RuleAction::RequireApproval,
            verified: true,
        });
    }

    Ok(())
}

fn unique_action_name(
    server_id: &str,
    kind: &str,
    name: &str,
    used: &mut HashMap<String, usize>,
) -> String {
    let base = format!(
        "mcp_{}_{}_{}",
        &server_id.chars().take(8).collect::<String>(),
        kind,
        normalize_segment(name)
    );
    let mut candidate = enforce_action_length(&base, name);
    if let Some(count) = used.get_mut(&candidate) {
        *count += 1;
        candidate = enforce_action_length(&format!("{}_{}", candidate, count), name);
    } else {
        used.insert(candidate.clone(), 1);
    }
    candidate
}

fn normalize_segment(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        "tool".to_string()
    } else {
        out
    }
}

fn enforce_action_length(base: &str, seed: &str) -> String {
    const MAX_LEN: usize = 64;
    if base.len() <= MAX_LEN {
        return base.to_string();
    }
    let hash = blake3::hash(seed.as_bytes()).to_hex();
    let suffix = &hash[..6];
    let mut trimmed: String = base.chars().take(MAX_LEN - 7).collect();
    trimmed.push('_');
    trimmed.push_str(suffix);
    trimmed
}

fn format_mcp_result(result: &Value) -> String {
    let is_error = result
        .get("isError")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if let Some(items) = result.get("content").and_then(|v| v.as_array()) {
        let mut parts = Vec::new();
        for item in items {
            if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                parts.push(text.to_string());
                continue;
            }
            if let Some(mime) = item.get("mimeType").and_then(|v| v.as_str()) {
                parts.push(format!("[MCP_CONTENT {}]", mime));
                continue;
            }
            parts.push(item.to_string());
        }
        let combined = parts.join("\n");
        if is_error {
            return format!("MCP Error:\n{}", combined);
        }
        return combined;
    }

    if let Some(contents) = result.get("contents").and_then(|v| v.as_array()) {
        let mut parts = Vec::new();
        for item in contents {
            if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                parts.push(text.to_string());
            } else {
                parts.push(item.to_string());
            }
        }
        let combined = parts.join("\n");
        if is_error {
            return format!("MCP Error:\n{}", combined);
        }
        return combined;
    }

    if let Some(text) = result.get("text").and_then(|v| v.as_str()) {
        return if is_error {
            format!("MCP Error:\n{}", text)
        } else {
            text.to_string()
        };
    }

    let fallback = serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string());
    if is_error {
        format!("MCP Error:\n{}", fallback)
    } else {
        fallback
    }
}
