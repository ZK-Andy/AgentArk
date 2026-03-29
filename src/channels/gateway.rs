//! Channel gateway foundation.
//!
//! This module defines channel metadata, adapter traits, and a lightweight
//! registry that can be extended with real transports later. It intentionally
//! stops at descriptors and runtime status scaffolding.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Canonical channel kinds supported by the gateway layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ChannelKind {
    #[serde(rename = "webchat")]
    WebChat,
    #[serde(rename = "telegram")]
    Telegram,
    #[serde(rename = "whatsapp")]
    WhatsApp,
    #[serde(rename = "slack")]
    Slack,
    #[serde(rename = "discord")]
    Discord,
    #[serde(rename = "matrix")]
    Matrix,
    #[serde(rename = "teams")]
    Teams,
    #[serde(rename = "google_chat")]
    GoogleChat,
    #[serde(rename = "signal")]
    Signal,
    #[serde(rename = "imessage")]
    IMessage,
}

impl ChannelKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WebChat => "webchat",
            Self::Telegram => "telegram",
            Self::WhatsApp => "whatsapp",
            Self::Slack => "slack",
            Self::Discord => "discord",
            Self::Matrix => "matrix",
            Self::Teams => "teams",
            Self::GoogleChat => "google_chat",
            Self::Signal => "signal",
            Self::IMessage => "imessage",
        }
    }
}

/// What kind of transport or bridge backs a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelTransportKind {
    Native,
    Bridge,
    Node,
    Plugin,
    Web,
}

/// What the gateway can do with a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelCapability {
    Inbound,
    Outbound,
    Realtime,
    Threads,
    Groups,
    DirectMessages,
    Attachments,
    ReadReceipts,
    Presence,
    Voice,
    ScreenShare,
    Location,
    InteractiveButtons,
}

/// Lifecycle state for a channel adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelRuntimeState {
    Planned,
    Configured,
    Connecting,
    Ready,
    Degraded,
    Disabled,
    Error,
}

/// Transport metadata for a channel adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelTransport {
    pub kind: ChannelTransportKind,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature_flag: Option<String>,
}

/// Setup hints for a channel adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelSetup {
    pub kind: String,
    pub title: String,
    pub summary: String,
    #[serde(default)]
    pub steps: Vec<String>,
}

/// Static metadata about a channel adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelAdapterDescriptor {
    pub kind: ChannelKind,
    pub name: String,
    pub display_name: String,
    pub summary: String,
    pub transport: ChannelTransport,
    #[serde(default)]
    pub capabilities: Vec<ChannelCapability>,
    #[serde(default)]
    pub setup: Vec<ChannelSetup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing_scope_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integration_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

impl ChannelAdapterDescriptor {
    pub fn supports(&self, capability: ChannelCapability) -> bool {
        self.capabilities.contains(&capability)
    }
}

/// Runtime status for a configured adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelRuntimeStatus {
    pub state: ChannelRuntimeState,
    pub connected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_checked_at: Option<String>,
    #[serde(default)]
    pub details: BTreeMap<String, String>,
}

impl ChannelRuntimeStatus {
    pub fn planned() -> Self {
        Self {
            state: ChannelRuntimeState::Planned,
            connected: false,
            last_error: None,
            last_checked_at: None,
            details: BTreeMap::new(),
        }
    }

    pub fn ready() -> Self {
        Self {
            state: ChannelRuntimeState::Ready,
            connected: true,
            last_error: None,
            last_checked_at: None,
            details: BTreeMap::new(),
        }
    }
}

/// Adapter behavior contract.
pub trait ChannelAdapter: Send + Sync {
    fn descriptor(&self) -> &ChannelAdapterDescriptor;
    fn status(&self) -> ChannelRuntimeStatus;

    fn kind(&self) -> ChannelKind {
        self.descriptor().kind
    }

    fn is_enabled(&self) -> bool {
        !matches!(self.status().state, ChannelRuntimeState::Disabled)
    }
}

/// A lightweight adapter stub that exposes only metadata and a fixed status.
#[derive(Debug, Clone)]
pub struct StaticChannelAdapter {
    descriptor: ChannelAdapterDescriptor,
    status: ChannelRuntimeStatus,
}

impl StaticChannelAdapter {
    pub fn new(descriptor: ChannelAdapterDescriptor, status: ChannelRuntimeStatus) -> Self {
        Self { descriptor, status }
    }

    pub fn descriptor_owned(&self) -> ChannelAdapterDescriptor {
        self.descriptor.clone()
    }
}

impl ChannelAdapter for StaticChannelAdapter {
    fn descriptor(&self) -> &ChannelAdapterDescriptor {
        &self.descriptor
    }

    fn status(&self) -> ChannelRuntimeStatus {
        self.status.clone()
    }
}

/// Registry of channel adapters used by the gateway control plane.
pub struct ChannelGatewayRegistry {
    adapters: BTreeMap<ChannelKind, Box<dyn ChannelAdapter>>,
}

impl Default for ChannelGatewayRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelGatewayRegistry {
    pub fn new() -> Self {
        let mut adapters: BTreeMap<ChannelKind, Box<dyn ChannelAdapter>> = BTreeMap::new();
        for adapter in default_channel_adapters() {
            adapters.insert(adapter.kind(), Box::new(adapter));
        }
        Self { adapters }
    }

    pub fn list_descriptors(&self) -> Vec<ChannelAdapterDescriptor> {
        self.adapters
            .values()
            .map(|adapter| adapter.descriptor().clone())
            .collect()
    }

    pub fn list_statuses(&self) -> Vec<ChannelStatusView> {
        self.adapters
            .values()
            .map(|adapter| ChannelStatusView {
                kind: adapter.kind(),
                descriptor: adapter.descriptor().clone(),
                status: adapter.status(),
            })
            .collect()
    }

    pub fn get(&self, kind: ChannelKind) -> Option<&dyn ChannelAdapter> {
        self.adapters.get(&kind).map(|adapter| adapter.as_ref())
    }
}

/// A view model that combines descriptor and runtime state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelStatusView {
    pub kind: ChannelKind,
    pub descriptor: ChannelAdapterDescriptor,
    pub status: ChannelRuntimeStatus,
}

#[allow(clippy::too_many_arguments)]
fn channel_descriptor(
    kind: ChannelKind,
    display_name: &str,
    summary: &str,
    transport: ChannelTransport,
    capabilities: Vec<ChannelCapability>,
    routing_scope_hint: Option<&str>,
    docs_url: Option<&str>,
    setup_url: Option<&str>,
    credential_model: Option<&str>,
    integration_model: Option<&str>,
    setup: Vec<ChannelSetup>,
    notes: Option<&str>,
) -> ChannelAdapterDescriptor {
    ChannelAdapterDescriptor {
        kind,
        name: kind.as_str().to_string(),
        display_name: display_name.to_string(),
        summary: summary.to_string(),
        transport,
        capabilities,
        setup,
        routing_scope_hint: routing_scope_hint.map(|value| value.to_string()),
        docs_url: docs_url.map(|value| value.to_string()),
        setup_url: setup_url.map(|value| value.to_string()),
        credential_model: credential_model.map(|value| value.to_string()),
        integration_model: integration_model.map(|value| value.to_string()),
        notes: notes.map(|value| value.to_string()),
    }
}

fn docs_url_for(kind: ChannelKind) -> Option<&'static str> {
    match kind {
        ChannelKind::WebChat => Some("https://docs.openclaw.ai/channels/webchat"),
        ChannelKind::Telegram => Some("https://docs.openclaw.ai/channels/telegram"),
        ChannelKind::WhatsApp => Some("https://docs.openclaw.ai/channels/whatsapp"),
        ChannelKind::Slack => Some("https://docs.openclaw.ai/channels/slack"),
        ChannelKind::Discord => Some("https://docs.openclaw.ai/channels/discord"),
        ChannelKind::Matrix => Some("https://docs.openclaw.ai/channels/matrix"),
        ChannelKind::Teams => Some("https://docs.openclaw.ai/channels/teams"),
        ChannelKind::GoogleChat => Some("https://docs.openclaw.ai/channels/google-chat"),
        ChannelKind::Signal => Some("https://docs.openclaw.ai/channels/signal"),
        ChannelKind::IMessage => Some("https://docs.openclaw.ai/channels/imessage"),
    }
}

fn setup_url_for(kind: ChannelKind) -> Option<&'static str> {
    match kind {
        ChannelKind::WebChat => Some("https://docs.openclaw.ai/channels/webchat/setup"),
        ChannelKind::Telegram => Some("https://docs.openclaw.ai/channels/telegram/setup"),
        ChannelKind::WhatsApp => Some("https://docs.openclaw.ai/channels/whatsapp/setup"),
        ChannelKind::Slack => Some("https://docs.openclaw.ai/channels/slack/setup"),
        ChannelKind::Discord => Some("https://docs.openclaw.ai/channels/discord/setup"),
        ChannelKind::Matrix => Some("https://docs.openclaw.ai/channels/matrix/setup"),
        ChannelKind::Teams => Some("https://docs.openclaw.ai/channels/teams/setup"),
        ChannelKind::GoogleChat => Some("https://docs.openclaw.ai/channels/google-chat/setup"),
        ChannelKind::Signal => Some("https://docs.openclaw.ai/channels/signal/setup"),
        ChannelKind::IMessage => Some("https://docs.openclaw.ai/channels/imessage/setup"),
    }
}

fn default_channel_adapters() -> Vec<StaticChannelAdapter> {
    vec![
        StaticChannelAdapter::new(
            channel_descriptor(
                ChannelKind::WebChat,
                "WebChat",
                "Native browser-facing chat surface for the control plane.",
                ChannelTransport {
                    kind: ChannelTransportKind::Web,
                    description: "Built-in web UI and session-backed chat surface".to_string(),
                    bridge_name: Some("browser_session".to_string()),
                    feature_flag: None,
                },
                vec![
                    ChannelCapability::Inbound,
                    ChannelCapability::Outbound,
                    ChannelCapability::Realtime,
                    ChannelCapability::Attachments,
                    ChannelCapability::InteractiveButtons,
                ],
                Some("per_channel"),
                docs_url_for(ChannelKind::WebChat),
                setup_url_for(ChannelKind::WebChat),
                Some("session"),
                Some("native"),
                vec![ChannelSetup {
                    kind: "local".to_string(),
                    title: "Local web session".to_string(),
                    summary: "Uses the existing authenticated UI session and HTTP API.".to_string(),
                    steps: vec![
                        "Open the web UI".to_string(),
                        "Authenticate the session".to_string(),
                        "Send messages through the chat pane".to_string(),
                    ],
                }],
                Some("Already implemented elsewhere in the product; this adapter surfaces it as a gateway target."),
            ),
            ChannelRuntimeStatus::ready(),
        ),
        StaticChannelAdapter::new(
            channel_descriptor(
                ChannelKind::Telegram,
                "Telegram",
                "Telegram bot channel with allowlist and pairing controls.",
                ChannelTransport {
                    kind: ChannelTransportKind::Native,
                    description: "Teloxide-backed bot integration".to_string(),
                    bridge_name: Some("telegram".to_string()),
                    feature_flag: Some("telegram".to_string()),
                },
                vec![
                    ChannelCapability::Inbound,
                    ChannelCapability::Outbound,
                    ChannelCapability::Realtime,
                    ChannelCapability::Attachments,
                    ChannelCapability::ReadReceipts,
                    ChannelCapability::InteractiveButtons,
                ],
                Some("per_channel"),
                docs_url_for(ChannelKind::Telegram),
                setup_url_for(ChannelKind::Telegram),
                Some("bot_token"),
                Some("native"),
                vec![ChannelSetup {
                    kind: "bot".to_string(),
                    title: "Bot token and allowlist".to_string(),
                    summary: "Configure the bot token, then restrict delivery with pairing or allowlists.".to_string(),
                    steps: vec![
                        "Paste the bot token".to_string(),
                        "Configure allowed users or pairing mode".to_string(),
                        "Verify outbound delivery".to_string(),
                    ],
                }],
                Some("Existing native channel in the codebase; included here so the gateway has a canonical descriptor."),
            ),
            ChannelRuntimeStatus::ready(),
        ),
        StaticChannelAdapter::new(
            channel_descriptor(
                ChannelKind::WhatsApp,
                "WhatsApp",
                "WhatsApp bridge channel with pairing and DM policy controls.",
                ChannelTransport {
                    kind: ChannelTransportKind::Bridge,
                    description: "Bridge-backed WhatsApp delivery path".to_string(),
                    bridge_name: Some("whatsapp_bridge".to_string()),
                    feature_flag: None,
                },
                vec![
                    ChannelCapability::Inbound,
                    ChannelCapability::Outbound,
                    ChannelCapability::Realtime,
                    ChannelCapability::Attachments,
                    ChannelCapability::InteractiveButtons,
                    ChannelCapability::Presence,
                ],
                Some("per_channel"),
                docs_url_for(ChannelKind::WhatsApp),
                setup_url_for(ChannelKind::WhatsApp),
                Some("bridge_or_cloud_api"),
                Some("bridge"),
                vec![ChannelSetup {
                    kind: "bridge".to_string(),
                    title: "Bridge and pairing".to_string(),
                    summary: "Choose Cloud API or bridge mode, then configure DM policy and allowed numbers.".to_string(),
                    steps: vec![
                        "Select bridge mode".to_string(),
                        "Provide credentials or QR pairing".to_string(),
                        "Set DM policy and allowed numbers".to_string(),
                    ],
                }],
                Some("Existing native channel in the codebase; included here so routing and registry can expose it uniformly."),
            ),
            ChannelRuntimeStatus::ready(),
        ),
        StaticChannelAdapter::new(
            channel_descriptor(
                ChannelKind::Slack,
                "Slack",
                "Slack workspace connector with signed webhook ingress and Web API delivery.",
                ChannelTransport {
                    kind: ChannelTransportKind::Native,
                    description: "Signed Events API webhook with Web API replies".to_string(),
                    bridge_name: None,
                    feature_flag: None,
                },
                vec![
                    ChannelCapability::Inbound,
                    ChannelCapability::Outbound,
                    ChannelCapability::Realtime,
                    ChannelCapability::Threads,
                    ChannelCapability::Groups,
                    ChannelCapability::Attachments,
                    ChannelCapability::ReadReceipts,
                    ChannelCapability::InteractiveButtons,
                ],
                Some("team"),
                docs_url_for(ChannelKind::Slack),
                setup_url_for(ChannelKind::Slack),
                Some("workspace_install"),
                Some("plugin"),
                vec![ChannelSetup {
                    kind: "workspace".to_string(),
                    title: "Workspace app".to_string(),
                    summary: "Register a Slack app and connect workspace authorization.".to_string(),
                    steps: vec![
                        "Create or install the app".to_string(),
                        "Authorize the workspace".to_string(),
                        "Bind threads or channels to routes".to_string(),
                    ],
                }],
                Some("Live transport: signed inbound events, thread-aware routing, outbound replies via chat.postMessage."),
            ),
            ChannelRuntimeStatus::ready(),
        ),
        StaticChannelAdapter::new(
            channel_descriptor(
                ChannelKind::Discord,
                "Discord",
                "Discord guild and DM connector backed by Gateway + REST delivery.",
                ChannelTransport {
                    kind: ChannelTransportKind::Native,
                    description: "Gateway websocket ingress with REST/webhook replies".to_string(),
                    bridge_name: None,
                    feature_flag: None,
                },
                vec![
                    ChannelCapability::Inbound,
                    ChannelCapability::Outbound,
                    ChannelCapability::Realtime,
                    ChannelCapability::Threads,
                    ChannelCapability::Groups,
                    ChannelCapability::Attachments,
                    ChannelCapability::InteractiveButtons,
                ],
                Some("guild"),
                docs_url_for(ChannelKind::Discord),
                setup_url_for(ChannelKind::Discord),
                Some("bot_token"),
                Some("plugin"),
                vec![ChannelSetup {
                    kind: "bot".to_string(),
                    title: "Bot and guild install".to_string(),
                    summary: "Install a Discord bot and map guild channels to routes.".to_string(),
                    steps: vec![
                        "Create a Discord application".to_string(),
                        "Install the bot in a guild".to_string(),
                        "Map channels or threads to routes".to_string(),
                    ],
                }],
                Some("Live transport: Gateway websocket session with reconnect/resume and outbound channel delivery."),
            ),
            ChannelRuntimeStatus::ready(),
        ),
        StaticChannelAdapter::new(
            channel_descriptor(
                ChannelKind::Matrix,
                "Matrix",
                "Matrix homeserver connector with sync polling and room delivery.",
                ChannelTransport {
                    kind: ChannelTransportKind::Native,
                    description: "Client sync API with outbound room message delivery".to_string(),
                    bridge_name: None,
                    feature_flag: None,
                },
                vec![
                    ChannelCapability::Inbound,
                    ChannelCapability::Outbound,
                    ChannelCapability::Realtime,
                    ChannelCapability::Threads,
                    ChannelCapability::Groups,
                    ChannelCapability::Attachments,
                ],
                Some("room"),
                docs_url_for(ChannelKind::Matrix),
                setup_url_for(ChannelKind::Matrix),
                Some("homeserver_or_bridge"),
                Some("plugin"),
                vec![ChannelSetup {
                    kind: "homeserver".to_string(),
                    title: "Homeserver or bridge".to_string(),
                    summary: "Connect to a Matrix homeserver or a bridge-backed room mapping.".to_string(),
                    steps: vec![
                        "Choose homeserver credentials".to_string(),
                        "Join or map rooms".to_string(),
                        "Assign room-level routes".to_string(),
                    ],
                }],
                Some("Live transport: sync polling, room/thread routing, and outbound room replies."),
            ),
            ChannelRuntimeStatus::ready(),
        ),
        StaticChannelAdapter::new(
            channel_descriptor(
                ChannelKind::Teams,
                "Teams",
                "Microsoft Teams connector for Bot Framework and Graph-backed delivery.",
                ChannelTransport {
                    kind: ChannelTransportKind::Native,
                    description: "Inbound activity webhook with Bot Framework / Graph replies".to_string(),
                    bridge_name: None,
                    feature_flag: None,
                },
                vec![
                    ChannelCapability::Inbound,
                    ChannelCapability::Outbound,
                    ChannelCapability::Realtime,
                    ChannelCapability::Threads,
                    ChannelCapability::Groups,
                    ChannelCapability::Attachments,
                    ChannelCapability::InteractiveButtons,
                ],
                Some("team"),
                docs_url_for(ChannelKind::Teams),
                setup_url_for(ChannelKind::Teams),
                Some("tenant_admin"),
                Some("plugin"),
                vec![ChannelSetup {
                    kind: "tenant".to_string(),
                    title: "Tenant app registration".to_string(),
                    summary: "Register a Teams app and map tenants or teams to routes.".to_string(),
                    steps: vec![
                        "Register the app".to_string(),
                        "Approve tenant access".to_string(),
                        "Bind teams or threads to routes".to_string(),
                    ],
                }],
                Some("Live transport: activity ingestion with persisted reply destinations and outbound follow-up delivery."),
            ),
            ChannelRuntimeStatus::ready(),
        ),
        StaticChannelAdapter::new(
            channel_descriptor(
                ChannelKind::GoogleChat,
                "Google Chat",
                "Google Chat space connector placeholder.",
                ChannelTransport {
                    kind: ChannelTransportKind::Plugin,
                    description: "Future Google Chat adapter or plugin transport".to_string(),
                    bridge_name: None,
                    feature_flag: None,
                },
                vec![
                    ChannelCapability::Inbound,
                    ChannelCapability::Outbound,
                    ChannelCapability::Realtime,
                    ChannelCapability::Threads,
                    ChannelCapability::Groups,
                    ChannelCapability::Attachments,
                ],
                Some("space"),
                docs_url_for(ChannelKind::GoogleChat),
                setup_url_for(ChannelKind::GoogleChat),
                Some("workspace_install"),
                Some("plugin"),
                vec![ChannelSetup {
                    kind: "workspace".to_string(),
                    title: "Workspace app".to_string(),
                    summary: "Configure Google Chat app credentials and bind spaces.".to_string(),
                    steps: vec![
                        "Create the chat app".to_string(),
                        "Authorize the workspace".to_string(),
                        "Bind spaces or threads to routes".to_string(),
                    ],
                }],
                Some("Stub descriptor only; transport will be added in a later milestone."),
            ),
            ChannelRuntimeStatus::planned(),
        ),
        StaticChannelAdapter::new(
            channel_descriptor(
                ChannelKind::Signal,
                "Signal",
                "Signal connector placeholder.",
                ChannelTransport {
                    kind: ChannelTransportKind::Bridge,
                    description: "Future Signal bridge transport".to_string(),
                    bridge_name: None,
                    feature_flag: None,
                },
                vec![
                    ChannelCapability::Inbound,
                    ChannelCapability::Outbound,
                    ChannelCapability::Realtime,
                    ChannelCapability::Attachments,
                    ChannelCapability::Presence,
                ],
                Some("peer"),
                docs_url_for(ChannelKind::Signal),
                setup_url_for(ChannelKind::Signal),
                Some("bridge_identity"),
                Some("bridge"),
                vec![ChannelSetup {
                    kind: "bridge".to_string(),
                    title: "Signal bridge".to_string(),
                    summary: "Connect via a dedicated bridge or node-backed relay.".to_string(),
                    steps: vec![
                        "Provision the bridge".to_string(),
                        "Register a paired identity".to_string(),
                        "Bind peer routes".to_string(),
                    ],
                }],
                Some("Stub descriptor only; transport will be added in a later milestone."),
            ),
            ChannelRuntimeStatus::planned(),
        ),
        StaticChannelAdapter::new(
            channel_descriptor(
                ChannelKind::IMessage,
                "iMessage",
                "iMessage / BlueBubbles connector placeholder.",
                ChannelTransport {
                    kind: ChannelTransportKind::Node,
                    description: "Future device or node-backed iMessage transport".to_string(),
                    bridge_name: None,
                    feature_flag: None,
                },
                vec![
                    ChannelCapability::Inbound,
                    ChannelCapability::Outbound,
                    ChannelCapability::Realtime,
                    ChannelCapability::Attachments,
                    ChannelCapability::Presence,
                ],
                Some("peer"),
                docs_url_for(ChannelKind::IMessage),
                setup_url_for(ChannelKind::IMessage),
                Some("node_pairing"),
                Some("node"),
                vec![ChannelSetup {
                    kind: "node".to_string(),
                    title: "macOS or companion node".to_string(),
                    summary: "Pair a companion node or relay to expose iMessage safely.".to_string(),
                    steps: vec![
                        "Pair the companion node".to_string(),
                        "Grant the required device permission".to_string(),
                        "Bind peers to the node route".to_string(),
                    ],
                }],
                Some("Stub descriptor only; transport will be added in a later milestone."),
            ),
            ChannelRuntimeStatus::planned(),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_contains_all_descriptors() {
        let registry = ChannelGatewayRegistry::new();
        let kinds: Vec<_> = registry
            .list_descriptors()
            .into_iter()
            .map(|descriptor| descriptor.kind)
            .collect();
        assert_eq!(kinds.len(), 10);
        assert!(kinds.contains(&ChannelKind::WebChat));
        assert!(kinds.contains(&ChannelKind::Telegram));
        assert!(kinds.contains(&ChannelKind::WhatsApp));
        assert!(kinds.contains(&ChannelKind::Slack));
        assert!(kinds.contains(&ChannelKind::Discord));
        assert!(kinds.contains(&ChannelKind::Matrix));
        assert!(kinds.contains(&ChannelKind::Teams));
        assert!(kinds.contains(&ChannelKind::GoogleChat));
        assert!(kinds.contains(&ChannelKind::Signal));
        assert!(kinds.contains(&ChannelKind::IMessage));
    }

    #[test]
    fn channel_descriptors_expose_routing_hints() {
        let registry = ChannelGatewayRegistry::new();
        let slack = registry.get(ChannelKind::Slack).expect("slack descriptor");
        assert_eq!(
            slack.descriptor().routing_scope_hint.as_deref(),
            Some("team")
        );
        assert!(slack.descriptor().supports(ChannelCapability::Threads));
    }
}
