//! Semantic DAG router.
//!
//! This module is the single routing authority for user turns. Classifier
//! output and semantic action scores are advisory inputs only; eligibility,
//! binding, policy, and the compiled `ExecutionDAG` decide what can run.

use crate::actions::{
    ActionDef, PlannerActionRole, PlannerCostTier, PlannerDeliveryMode, PlannerIntegrationClass,
    PlannerSideEffectLevel,
};
use crate::security::intent_classifier::{InboundRoutingSignal, InboundTurnGoal};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

pub const ROUTER_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CurrentSurface {
    Chat,
    Canvas,
    App,
    Browser,
    BackgroundSession,
    InternalAdmin,
}

impl Default for CurrentSurface {
    fn default() -> Self {
        Self::Chat
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct FocusedObjectRefs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_app_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_canvas_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_watcher_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_browser_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_deployment_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserMessageEnvelope {
    pub schema_version: u32,
    pub message_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    pub user_id: String,
    pub raw_text: String,
    pub normalized_text: String,
    pub timestamp: DateTime<Utc>,
    pub timezone: String,
    pub locale: String,
    pub current_surface: CurrentSurface,
    #[serde(default)]
    pub focused_object_refs: FocusedObjectRefs,
    #[serde(default)]
    pub attachment_refs: Vec<String>,
    #[serde(default)]
    pub pasted_context_refs: Vec<String>,
    #[serde(default)]
    pub previous_turn_refs: Vec<String>,
    #[serde(default)]
    pub safety_context: serde_json::Value,
}

impl UserMessageEnvelope {
    pub fn chat(
        raw_text: impl Into<String>,
        conversation_id: Option<&str>,
        user_id: impl Into<String>,
    ) -> Self {
        let raw_text = raw_text.into();
        Self {
            schema_version: ROUTER_SCHEMA_VERSION,
            message_id: uuid::Uuid::new_v4().to_string(),
            conversation_id: conversation_id
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned),
            user_id: user_id.into(),
            normalized_text: normalize_text(&raw_text),
            raw_text,
            timestamp: Utc::now(),
            timezone: "UTC".to_string(),
            locale: "en-US".to_string(),
            current_surface: CurrentSurface::Chat,
            focused_object_refs: FocusedObjectRefs::default(),
            attachment_refs: Vec::new(),
            pasted_context_refs: Vec::new(),
            previous_turn_refs: Vec::new(),
            safety_context: serde_json::json!({}),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthStatus {
    Authenticated,
    Expired,
    Missing,
    Revoked,
    Removed,
    InsufficientScope,
}

impl Default for AuthStatus {
    fn default() -> Self {
        Self::Missing
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct IntegrationSnapshot {
    pub integration_id: String,
    pub name: String,
    pub installed: bool,
    pub connected: bool,
    pub auth_status: AuthStatus,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_success_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    pub reconnect_available: bool,
    pub install_available: bool,
    pub remove_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct NotificationChannelSnapshot {
    pub channel_id: String,
    pub name: String,
    pub available: bool,
    pub configured: bool,
    pub authenticated: bool,
    pub supports_inline: bool,
    pub supports_rich_message: bool,
    pub supports_attachments: bool,
    pub supports_background_delivery: bool,
    pub fallback_allowed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_delivery_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeObjectType {
    Watcher,
    BackgroundSession,
    App,
    Artifact,
    Dashboard,
    Deployment,
    BrowserSession,
    Integration,
    NotificationChannel,
    Memory,
    InternalState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RuntimeObjectSnapshot {
    pub object_type: RuntimeObjectType,
    pub object_id: String,
    pub name: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_conversation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_touched_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl Default for RuntimeObjectType {
    fn default() -> Self {
        Self::InternalState
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionSnapshot {
    pub action_id: String,
    pub name: String,
    pub input_schema: serde_json::Value,
    #[serde(default)]
    pub output_schema: serde_json::Value,
    pub role: PlannerActionRole,
    pub integration_class: PlannerIntegrationClass,
    pub side_effect_level: PlannerSideEffectLevel,
    pub required_auth: bool,
    #[serde(default)]
    pub required_scopes: Vec<String>,
    #[serde(default)]
    pub execution_surfaces: Vec<CurrentSurface>,
    pub capability_contract_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeStateSnapshot {
    pub schema_version: u32,
    pub snapshot_id: String,
    pub captured_at: DateTime<Utc>,
    #[serde(default)]
    pub integrations: Vec<IntegrationSnapshot>,
    #[serde(default)]
    pub notification_channels: Vec<NotificationChannelSnapshot>,
    #[serde(default)]
    pub runtime_objects: Vec<RuntimeObjectSnapshot>,
    #[serde(default)]
    pub internal_state: serde_json::Value,
    #[serde(default)]
    pub available_actions: Vec<ActionSnapshot>,
    #[serde(default)]
    pub provider_errors: Vec<String>,
}

impl RuntimeStateSnapshot {
    pub fn from_actions(actions: &[ActionDef]) -> Self {
        let mut integrations = BTreeMap::<String, IntegrationSnapshot>::new();
        for action in actions {
            let metadata = action.planner_metadata();
            for integration_id in &action.authorization.access.integration_ids {
                let integration_id = integration_id.trim();
                if integration_id.is_empty() {
                    continue;
                }
                let entry = integrations
                    .entry(integration_id.to_string())
                    .or_insert_with(|| IntegrationSnapshot {
                        integration_id: integration_id.to_string(),
                        name: integration_id.to_string(),
                        installed: true,
                        connected: true,
                        auth_status: AuthStatus::Authenticated,
                        scopes: Vec::new(),
                        last_success_at: None,
                        last_error: None,
                        reconnect_available: true,
                        install_available: false,
                        remove_available: true,
                    });
                if metadata.requires_auth || action.authorization.requires_auth {
                    entry.auth_status = AuthStatus::Authenticated;
                }
                for scope in action.authorization.access.permission_ids.iter().chain(
                    action
                        .authorization
                        .access
                        .integration_features
                        .values()
                        .flatten(),
                ) {
                    if !entry.scopes.iter().any(|existing| existing == scope) {
                        entry.scopes.push(scope.clone());
                    }
                }
            }
        }
        Self {
            schema_version: ROUTER_SCHEMA_VERSION,
            snapshot_id: uuid::Uuid::new_v4().to_string(),
            captured_at: Utc::now(),
            integrations: integrations.into_values().collect(),
            notification_channels: Vec::new(),
            runtime_objects: Vec::new(),
            internal_state: serde_json::json!({}),
            available_actions: actions.iter().map(ActionSnapshot::from_action).collect(),
            provider_errors: Vec::new(),
        }
    }

    pub fn add_runtime_object(&mut self, object: RuntimeObjectSnapshot) {
        self.runtime_objects.push(object);
    }
}

impl ActionSnapshot {
    pub fn from_action(action: &ActionDef) -> Self {
        let metadata = action.planner_metadata();
        Self {
            action_id: action.name.clone(),
            name: action.name.clone(),
            input_schema: action.input_schema.clone(),
            output_schema: serde_json::json!({}),
            role: metadata.role,
            integration_class: metadata.integration_class,
            side_effect_level: metadata.side_effect_level,
            required_auth: metadata.requires_auth || action.authorization.requires_auth,
            required_scopes: action
                .authorization
                .access
                .permission_ids
                .iter()
                .chain(
                    action
                        .authorization
                        .access
                        .integration_features
                        .values()
                        .flatten(),
                )
                .cloned()
                .collect(),
            execution_surfaces: vec![CurrentSurface::Chat],
            capability_contract_id: capability_contract_id_for_action(action),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum IntentType {
    CasualChat,
    CapabilityExplanation,
    AnswerQuestion,
    MemoryRecall,
    CurrentStatusQuery,
    InternalSystemQuery,
    ArkPulseQuery,
    ArkSentinelQuery,
    TraceQuery,
    PrivateIntegrationQuery,
    PublicWebResearch,
    DeepResearch,
    AppCreate,
    AppEdit,
    AppDeploy,
    AppExplain,
    WatcherCreate,
    WatcherUpdate,
    WatcherPause,
    WatcherResume,
    WatcherStop,
    WatcherDelete,
    ReminderCreate,
    CalendarEventCreate,
    NotificationChannelUpdate,
    IntegrationInstall,
    IntegrationAuthReconnect,
    IntegrationRemove,
    BrowserAutomation,
    FileOperation,
    CanvasOperation,
    Delegation,
    ClarificationNeeded,
    SafetyRefusal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteDependencyEdge {
    pub from_node_id: String,
    pub to_node_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RouteIntentNode {
    pub node_id: String,
    pub intent_type: IntentType,
    pub user_visible_goal: String,
    pub semantic_goal: String,
    pub read_only: bool,
    pub mutates_state: bool,
    pub destructive: bool,
    pub requires_confirmation: bool,
    pub temporal_scope: Option<String>,
    pub target_object_hint: Option<String>,
    pub target_object_type: Option<RuntimeObjectType>,
    pub target_object_id: Option<String>,
    pub creates_new_object: bool,
    #[serde(default)]
    pub likely_required_capabilities: Vec<String>,
    pub required_data_source_hint: Option<String>,
    pub required_notification_hint: Option<String>,
    pub required_deployment_hint: Option<String>,
    #[serde(default)]
    pub missing_inputs: Vec<String>,
    #[serde(default)]
    pub safety_flags: Vec<String>,
    pub confidence: f32,
    pub reason: String,
    pub decomposition_method: String,
    #[serde(default)]
    pub preconditions: Vec<String>,
    #[serde(default)]
    pub effects: Vec<String>,
    #[serde(default)]
    pub partial_order_constraints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentArkRouteDAG {
    pub schema_version: u32,
    pub route_dag_id: String,
    pub message_id: String,
    pub normalized_goal: String,
    pub nodes: Vec<RouteIntentNode>,
    pub edges: Vec<RouteDependencyEdge>,
    #[serde(default)]
    pub global_constraints: Vec<String>,
    pub ambiguity_level: String,
    #[serde(default)]
    pub missing_inputs: Vec<String>,
    #[serde(default)]
    pub safety_flags: Vec<String>,
    pub route_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteDAGValidationResult {
    pub valid: bool,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObjectResolutionCandidate {
    pub object_type: RuntimeObjectType,
    pub object_id: String,
    pub name: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_touched_at: Option<DateTime<Utc>>,
    pub relevance_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObjectResolutionResult {
    pub node_id: String,
    pub resolved: bool,
    pub target_object_type: Option<RuntimeObjectType>,
    pub target_object_id: Option<String>,
    pub confidence: f32,
    #[serde(default)]
    pub candidates: Vec<ObjectResolutionCandidate>,
    pub ambiguity_level: String,
    pub requires_clarification: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContractSideEffectLevel {
    AnswerOnly,
    ReadPublic,
    ReadPrivate,
    WriteLocal,
    MutateSession,
    DeployPublic,
    SendExternal,
    DeleteOrDestroy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContractDurability {
    OneShot,
    PersistentSession,
    RecurringWatcher,
    DeployedApp,
    LocalArtifact,
    CalendarEvent,
    Reminder,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityContract {
    pub contract_id: String,
    pub action_id: String,
    pub display_name: String,
    #[serde(default)]
    pub supported_intents: Vec<IntentType>,
    pub goal_contract: String,
    #[serde(default)]
    pub preconditions: Vec<String>,
    #[serde(default)]
    pub effects: Vec<String>,
    pub input_schema: serde_json::Value,
    #[serde(default)]
    pub output_schema: serde_json::Value,
    #[serde(default)]
    pub required_integrations: Vec<String>,
    #[serde(default)]
    pub required_scopes: Vec<String>,
    #[serde(default)]
    pub required_state: Vec<String>,
    #[serde(default)]
    pub required_execution_surface: Vec<CurrentSurface>,
    pub side_effect_level: ContractSideEffectLevel,
    pub durability: ContractDurability,
    pub read_only: bool,
    pub destructive: bool,
    pub idempotent: bool,
    pub open_world: bool,
    pub requires_network: bool,
    pub requires_private_data: bool,
    pub retry_safe: bool,
    pub requires_confirmation: bool,
    pub requires_resolved_target: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compensation_action: Option<String>,
    #[serde(default)]
    pub fallback_actions: Vec<String>,
    #[serde(default)]
    pub observability_tags: Vec<String>,
    pub metadata_role: PlannerActionRole,
    pub metadata_integration_class: PlannerIntegrationClass,
    pub metadata_cost: PlannerCostTier,
    pub metadata_delivery_mode: PlannerDeliveryMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EligibleAction {
    pub action_id: String,
    pub contract_id: String,
    pub confidence: f32,
    #[serde(default)]
    pub proposed_inputs: serde_json::Value,
    pub eligibility_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RejectedAction {
    pub action_id: String,
    pub contract_id: String,
    pub rejected_because: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EligibilityResult {
    pub node_id: String,
    #[serde(default)]
    pub eligible_actions: Vec<EligibleAction>,
    #[serde(default)]
    pub rejected_actions: Vec<RejectedAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_clarification: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_auth_action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_setup_action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityBindingStatus {
    Bound,
    ClarificationNeeded,
    AuthNeeded,
    SetupNeeded,
    ConfirmationNeeded,
    Refusal,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityBinding {
    pub node_id: String,
    pub status: CapabilityBindingStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_id: Option<String>,
    #[serde(default)]
    pub final_inputs: serde_json::Value,
    #[serde(default)]
    pub missing_inputs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_issue: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_issue: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmation_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason: Option<String>,
    pub binding_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecisionKind {
    Allow,
    Deny,
    RequireConfirmation,
    RequireAuth,
    RequireClarification,
    RequireSafeFallback,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyDecision {
    pub node_id: String,
    pub decision: PolicyDecisionKind,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionNodeStatus {
    Ready,
    Running,
    Succeeded,
    Failed,
    Skipped,
    WaitingForAuth,
    WaitingForClarification,
    WaitingForConfirmation,
    WaitingForExternalLogin,
    Retrying,
    Compensating,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionNode {
    pub node_id: String,
    pub route_node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    pub status: ExecutionNodeStatus,
    #[serde(default)]
    pub inputs: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_output: Option<String>,
    pub read_only: bool,
    pub mutates_state: bool,
    pub destructive: bool,
    pub approval_required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_policy: Option<String>,
    #[serde(default)]
    pub fallback_node_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compensation_action: Option<String>,
    pub checkpoint_policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionDAG {
    pub schema_version: u32,
    pub execution_dag_id: String,
    pub route_dag_id: String,
    pub nodes: Vec<ExecutionNode>,
    pub edges: Vec<RouteDependencyEdge>,
    pub execution_order: Vec<String>,
    pub parallel_groups: Vec<Vec<String>>,
    pub requires_user_input_before_execution: bool,
    pub final_response_strategy: String,
}

impl ExecutionDAG {
    pub fn ready_action_ids(&self) -> BTreeSet<String> {
        self.nodes
            .iter()
            .filter(|node| node.status == ExecutionNodeStatus::Ready)
            .filter_map(|node| node.action_id.clone())
            .collect()
    }

    pub fn has_ready_action(&self, action_id: &str) -> bool {
        self.ready_action_ids().contains(action_id)
    }

    #[cfg(test)]
    pub fn ready_nodes_all_read_only(&self) -> bool {
        let ready = self
            .nodes
            .iter()
            .filter(|node| node.status == ExecutionNodeStatus::Ready)
            .collect::<Vec<_>>();
        !ready.is_empty() && ready.iter().all(|node| node.read_only)
    }
}

impl AgentArkRouteDAG {
    #[cfg(test)]
    pub fn all_nodes_read_only(&self) -> bool {
        !self.nodes.is_empty()
            && self
                .nodes
                .iter()
                .all(|node| node.read_only && !node.mutates_state && !node.destructive)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RouteTrace {
    pub schema_version: u32,
    pub trace_id: String,
    pub user_message: UserMessageEnvelope,
    pub runtime_snapshot_summary: RuntimeStateSnapshot,
    pub route_dag: AgentArkRouteDAG,
    pub validation: RouteDAGValidationResult,
    pub object_resolution: Vec<ObjectResolutionResult>,
    pub eligibility: Vec<EligibilityResult>,
    pub capability_bindings: Vec<CapabilityBinding>,
    pub policy_decisions: Vec<PolicyDecision>,
    pub execution_dag: ExecutionDAG,
    #[serde(default)]
    pub executor_results: Vec<serde_json::Value>,
    pub final_response_strategy: String,
}

pub fn route_user_message(
    envelope: UserMessageEnvelope,
    snapshot: RuntimeStateSnapshot,
    advisory: Option<&InboundRoutingSignal>,
    actions: &[ActionDef],
    semantic_scores: &HashMap<String, f32>,
) -> RouteTrace {
    let contracts = actions
        .iter()
        .map(capability_contract_for_action)
        .collect::<Vec<_>>();
    let route_dag = build_route_dag(&envelope, &snapshot, advisory);
    let validation = validate_route_dag(&route_dag);
    let object_resolution = resolve_objects(&route_dag, &snapshot);
    let eligibility = run_eligibility(
        &route_dag,
        &snapshot,
        &contracts,
        &object_resolution,
        semantic_scores,
    );
    let capability_bindings =
        bind_capabilities(&route_dag, &eligibility, &contracts, semantic_scores);
    let policy_decisions = run_policy_gate(
        &route_dag,
        &capability_bindings,
        &contracts,
        &object_resolution,
    );
    let execution_dag = compile_execution_dag(
        &route_dag,
        &capability_bindings,
        &policy_decisions,
        &contracts,
    );
    let final_response_strategy = execution_dag.final_response_strategy.clone();

    RouteTrace {
        schema_version: ROUTER_SCHEMA_VERSION,
        trace_id: uuid::Uuid::new_v4().to_string(),
        user_message: envelope,
        runtime_snapshot_summary: snapshot,
        route_dag,
        validation,
        object_resolution,
        eligibility,
        capability_bindings,
        policy_decisions,
        execution_dag,
        executor_results: Vec::new(),
        final_response_strategy,
    }
}

pub fn capability_contract_id_for_action(action: &ActionDef) -> String {
    format!("capability_contract:{}", action.name.trim())
}

pub fn capability_contract_for_action(action: &ActionDef) -> CapabilityContract {
    let metadata = action.planner_metadata();
    let read_only = metadata.side_effect_level == PlannerSideEffectLevel::None;
    let destructive = action.name.ends_with("_delete") || action.name == "app_delete";
    let supported_intents = supported_intents_for_action(action, &metadata);
    let side_effect_level = contract_side_effect(action, &metadata);
    let durability = contract_durability(action, &metadata);

    CapabilityContract {
        contract_id: capability_contract_id_for_action(action),
        action_id: action.name.clone(),
        display_name: action.name.clone(),
        goal_contract: supported_intents
            .iter()
            .map(|intent| format!("{intent:?}"))
            .collect::<Vec<_>>()
            .join(","),
        supported_intents,
        preconditions: Vec::new(),
        effects: if read_only {
            vec!["read result".to_string()]
        } else {
            vec!["state change".to_string()]
        },
        input_schema: action.input_schema.clone(),
        output_schema: serde_json::json!({}),
        required_integrations: action.authorization.access.integration_ids.clone(),
        required_scopes: action
            .authorization
            .access
            .permission_ids
            .iter()
            .chain(
                action
                    .authorization
                    .access
                    .integration_features
                    .values()
                    .flatten(),
            )
            .cloned()
            .collect(),
        required_state: required_state_for_action(action),
        required_execution_surface: vec![CurrentSurface::Chat],
        side_effect_level,
        durability,
        read_only,
        destructive,
        idempotent: read_only,
        open_world: action_accepts_open_world_inputs(action),
        requires_network: matches!(
            metadata.integration_class,
            PlannerIntegrationClass::Search
                | PlannerIntegrationClass::Network
                | PlannerIntegrationClass::Workspace
                | PlannerIntegrationClass::Browser
        ),
        requires_private_data: metadata.requires_auth || action.authorization.requires_auth,
        retry_safe: read_only
            || matches!(metadata.side_effect_level, PlannerSideEffectLevel::Notify),
        requires_confirmation: destructive || action.authorization.human_approval.required,
        requires_resolved_target: action_requires_resolved_target(action),
        compensation_action: compensation_action_for_action(action),
        fallback_actions: Vec::new(),
        observability_tags: action.capabilities.clone(),
        metadata_role: metadata.role,
        metadata_integration_class: metadata.integration_class,
        metadata_cost: metadata.cost,
        metadata_delivery_mode: metadata.delivery_mode,
    }
}

fn normalize_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn normalize_label(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn side_effect_label(value: &str) -> &'static str {
    match normalize_label(value).as_str() {
        "notify" => "notify",
        "delete" | "delete_object" => "delete",
        "write" | "create" | "modify" | "create_object" | "modify_object" => "write",
        _ => "none",
    }
}

fn durability_label(value: &str) -> String {
    normalize_label(value)
}

fn goal_requires_durable_outcome(goal: &InboundTurnGoal) -> bool {
    let durability = durability_label(&goal.durability);
    !durability.is_empty()
        && !matches!(
            durability.as_str(),
            "none" | "ephemeral" | "session" | "current_answer"
        )
}

fn intent_type_from_goal(goal: &InboundTurnGoal) -> IntentType {
    let durability = durability_label(&goal.durability);
    let side_effect = side_effect_label(&goal.side_effect);
    let grounding_labels = goal
        .groundings
        .iter()
        .map(|value| normalize_label(value))
        .collect::<BTreeSet<_>>();

    if side_effect == "delete" {
        return match durability.as_str() {
            "integration" => IntentType::IntegrationRemove,
            "deployment" => IntentType::AppEdit,
            "background_session" => IntentType::WatcherStop,
            _ => IntentType::WatcherDelete,
        };
    }
    if goal_requires_durable_outcome(goal) || side_effect != "none" {
        return match durability.as_str() {
            "deployment" => IntentType::AppCreate,
            "artifact" => IntentType::FileOperation,
            "recurring_monitor" | "watcher" => IntentType::WatcherCreate,
            "scheduled_time" => IntentType::ReminderCreate,
            "integration" => IntentType::IntegrationInstall,
            "background_session" => IntentType::Delegation,
            _ if side_effect == "notify" => IntentType::NotificationChannelUpdate,
            _ if side_effect == "write" => IntentType::FileOperation,
            _ => IntentType::Delegation,
        };
    }
    if grounding_labels.contains("user_memory") || grounding_labels.contains("saved_user_facts") {
        return IntentType::MemoryRecall;
    }
    if grounding_labels.contains("external_info") || grounding_labels.contains("public_web") {
        return IntentType::PublicWebResearch;
    }
    if grounding_labels.contains("local_state") || grounding_labels.contains("live_state") {
        return IntentType::InternalSystemQuery;
    }
    if grounding_labels.contains("agentark_capabilities") {
        return IntentType::CapabilityExplanation;
    }
    IntentType::AnswerQuestion
}

fn node_from_goal(goal: &InboundTurnGoal, index: usize, message_id: &str) -> RouteIntentNode {
    let side_effect = side_effect_label(&goal.side_effect);
    let durable = goal_requires_durable_outcome(goal);
    let read_only = side_effect == "none" && !durable;
    let intent_type = intent_type_from_goal(goal);
    let user_visible_goal = first_non_empty([
        goal.intent_summary.as_str(),
        goal.expected_outcome.as_str(),
        goal.capability_query.as_str(),
    ])
    .to_string();
    let semantic_goal = first_non_empty([
        goal.capability_query.as_str(),
        goal.expected_outcome.as_str(),
        goal.intent_summary.as_str(),
    ])
    .to_string();
    let node_id = if goal.id.trim().is_empty() {
        format!("{message_id}:g{}", index + 1)
    } else {
        goal.id.trim().to_string()
    };
    let destructive = side_effect == "delete";
    let mutates_state = destructive || side_effect != "none" || durable;
    RouteIntentNode {
        node_id,
        intent_type,
        user_visible_goal,
        semantic_goal,
        read_only,
        mutates_state,
        destructive,
        requires_confirmation: destructive,
        temporal_scope: None,
        target_object_hint: None,
        target_object_type: target_type_for_durability(&goal.durability),
        target_object_id: None,
        creates_new_object: durable && !destructive,
        likely_required_capabilities: goal
            .groundings
            .iter()
            .chain(std::iter::once(&goal.capability_query))
            .filter_map(|value| {
                let value = value.trim();
                (!value.is_empty()).then(|| value.to_string())
            })
            .collect(),
        required_data_source_hint: goal
            .groundings
            .iter()
            .map(String::as_str)
            .map(str::trim)
            .find(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        required_notification_hint: None,
        required_deployment_hint: (durability_label(&goal.durability) == "deployment")
            .then(|| "managed_app_deployment".to_string()),
        missing_inputs: Vec::new(),
        safety_flags: Vec::new(),
        confidence: 0.75,
        reason: "constructed from advisory semantic goal; no tool binding performed".to_string(),
        decomposition_method: "advisory_semantic_goal".to_string(),
        preconditions: Vec::new(),
        effects: Vec::new(),
        partial_order_constraints: goal.dependencies.clone(),
    }
}

fn target_type_for_durability(value: &str) -> Option<RuntimeObjectType> {
    match durability_label(value).as_str() {
        "deployment" => Some(RuntimeObjectType::App),
        "artifact" => Some(RuntimeObjectType::Artifact),
        "recurring_monitor" | "watcher" => Some(RuntimeObjectType::Watcher),
        "integration" => Some(RuntimeObjectType::Integration),
        "background_session" => Some(RuntimeObjectType::BackgroundSession),
        _ => None,
    }
}

fn build_route_dag(
    envelope: &UserMessageEnvelope,
    _snapshot: &RuntimeStateSnapshot,
    advisory: Option<&InboundRoutingSignal>,
) -> AgentArkRouteDAG {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    if let Some(signal) = advisory {
        for (index, goal) in signal.goals.iter().enumerate() {
            let node = node_from_goal(goal, index, &envelope.message_id);
            for dependency in &goal.dependencies {
                if !dependency.trim().is_empty() {
                    edges.push(RouteDependencyEdge {
                        from_node_id: dependency.trim().to_string(),
                        to_node_id: node.node_id.clone(),
                        reason: "advisory semantic dependency".to_string(),
                    });
                }
            }
            if matches!(node.intent_type, IntentType::AppCreate) {
                let deploy_id = format!("{}:deploy", node.node_id);
                edges.push(RouteDependencyEdge {
                    from_node_id: node.node_id.clone(),
                    to_node_id: deploy_id.clone(),
                    reason: "managed deployment depends on a buildable app artifact".to_string(),
                });
                let mut deploy_node = node.clone();
                deploy_node.node_id = deploy_id;
                deploy_node.intent_type = IntentType::AppDeploy;
                deploy_node.user_visible_goal = format!("Deploy {}", node.user_visible_goal.trim())
                    .trim()
                    .to_string();
                deploy_node.semantic_goal = "deploy the created app artifact".to_string();
                deploy_node.creates_new_object = false;
                deploy_node.target_object_type = Some(RuntimeObjectType::App);
                nodes.push(node);
                nodes.push(deploy_node);
            } else {
                nodes.push(node);
            }
        }
    }

    if nodes.is_empty() {
        let intent_type = if advisory
            .map(|signal| signal.current_answer_expected)
            .unwrap_or(true)
        {
            IntentType::AnswerQuestion
        } else {
            IntentType::CasualChat
        };
        nodes.push(RouteIntentNode {
            node_id: "g1".to_string(),
            intent_type,
            user_visible_goal: envelope.normalized_text.clone(),
            semantic_goal: envelope.normalized_text.clone(),
            read_only: true,
            mutates_state: false,
            destructive: false,
            requires_confirmation: false,
            temporal_scope: None,
            target_object_hint: None,
            target_object_type: None,
            target_object_id: None,
            creates_new_object: false,
            likely_required_capabilities: Vec::new(),
            required_data_source_hint: None,
            required_notification_hint: None,
            required_deployment_hint: None,
            missing_inputs: Vec::new(),
            safety_flags: Vec::new(),
            confidence: 0.5,
            reason: "no executable advisory goals; answer-only route".to_string(),
            decomposition_method: "answer_only_default".to_string(),
            preconditions: Vec::new(),
            effects: Vec::new(),
            partial_order_constraints: Vec::new(),
        });
    }

    AgentArkRouteDAG {
        schema_version: ROUTER_SCHEMA_VERSION,
        route_dag_id: format!("route:{}", envelope.message_id),
        message_id: envelope.message_id.clone(),
        normalized_goal: nodes
            .iter()
            .map(|node| node.semantic_goal.as_str())
            .filter(|goal| !goal.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" | "),
        nodes,
        edges,
        global_constraints: Vec::new(),
        ambiguity_level: "low".to_string(),
        missing_inputs: Vec::new(),
        safety_flags: Vec::new(),
        route_reason: "AgentArkRouteDAG is the only user-intent representation".to_string(),
    }
}

fn validate_route_dag(route_dag: &AgentArkRouteDAG) -> RouteDAGValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let ids = route_dag
        .nodes
        .iter()
        .map(|node| node.node_id.as_str())
        .collect::<BTreeSet<_>>();
    for edge in &route_dag.edges {
        if !ids.contains(edge.from_node_id.as_str()) {
            errors.push(format!(
                "dependency source `{}` is missing from RouteDAG",
                edge.from_node_id
            ));
        }
        if !ids.contains(edge.to_node_id.as_str()) {
            errors.push(format!(
                "dependency target `{}` is missing from RouteDAG",
                edge.to_node_id
            ));
        }
    }
    if has_cycle(route_dag) {
        errors.push("RouteDAG contains a cycle".to_string());
    }
    for node in &route_dag.nodes {
        if node.read_only && node.mutates_state {
            errors.push(format!(
                "node `{}` is both read-only and mutating",
                node.node_id
            ));
        }
        if node.destructive && !node.requires_confirmation {
            errors.push(format!(
                "destructive node `{}` lacks confirmation requirement",
                node.node_id
            ));
        }
        if matches!(node.intent_type, IntentType::AppDeploy)
            && !route_dag
                .edges
                .iter()
                .any(|edge| edge.to_node_id == node.node_id)
            && node.target_object_id.is_none()
        {
            warnings.push(format!(
                "deploy node `{}` needs a created or resolved app target before execution",
                node.node_id
            ));
        }
    }
    RouteDAGValidationResult {
        valid: errors.is_empty(),
        errors,
        warnings,
    }
}

fn has_cycle(route_dag: &AgentArkRouteDAG) -> bool {
    let ids = route_dag
        .nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<BTreeSet<_>>();
    let mut incoming = ids
        .iter()
        .map(|id| (id.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = ids
        .iter()
        .map(|id| (id.clone(), Vec::<String>::new()))
        .collect::<BTreeMap<_, _>>();
    for edge in &route_dag.edges {
        if ids.contains(&edge.from_node_id) && ids.contains(&edge.to_node_id) {
            *incoming.entry(edge.to_node_id.clone()).or_default() += 1;
            outgoing
                .entry(edge.from_node_id.clone())
                .or_default()
                .push(edge.to_node_id.clone());
        }
    }
    let mut ready = incoming
        .iter()
        .filter_map(|(id, count)| (*count == 0).then(|| id.clone()))
        .collect::<VecDeque<_>>();
    let mut visited = 0usize;
    while let Some(id) = ready.pop_front() {
        visited += 1;
        for child in outgoing.get(&id).cloned().unwrap_or_default() {
            if let Some(count) = incoming.get_mut(&child) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    ready.push_back(child);
                }
            }
        }
    }
    visited != ids.len()
}

fn resolve_objects(
    route_dag: &AgentArkRouteDAG,
    snapshot: &RuntimeStateSnapshot,
) -> Vec<ObjectResolutionResult> {
    route_dag
        .nodes
        .iter()
        .map(|node| {
            let candidates = node
                .target_object_type
                .as_ref()
                .map(|target_type| {
                    snapshot
                        .runtime_objects
                        .iter()
                        .filter(|object| &object.object_type == target_type)
                        .map(|object| ObjectResolutionCandidate {
                            object_type: object.object_type.clone(),
                            object_id: object.object_id.clone(),
                            name: object.name.clone(),
                            status: object.status.clone(),
                            last_touched_at: object.last_touched_at,
                            relevance_reason: "runtime snapshot object type matches route target"
                                .to_string(),
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let exact_target = node.target_object_id.clone().or_else(|| {
                if node.creates_new_object {
                    None
                } else if candidates.len() == 1 {
                    candidates
                        .first()
                        .map(|candidate| candidate.object_id.clone())
                } else {
                    None
                }
            });
            let requires_clarification = node.destructive && exact_target.is_none();
            ObjectResolutionResult {
                node_id: node.node_id.clone(),
                resolved: exact_target.is_some()
                    || node.creates_new_object
                    || node.target_object_type.is_none(),
                target_object_type: node.target_object_type.clone(),
                target_object_id: exact_target,
                confidence: if candidates.len() == 1 { 0.85 } else { 0.0 },
                candidates,
                ambiguity_level: if requires_clarification {
                    "high".to_string()
                } else {
                    "low".to_string()
                },
                requires_clarification,
                reason: if requires_clarification {
                    "destructive mutation requires an exact resolved target".to_string()
                } else {
                    "object resolution completed from immutable runtime snapshot".to_string()
                },
            }
        })
        .collect()
}

fn run_eligibility(
    route_dag: &AgentArkRouteDAG,
    snapshot: &RuntimeStateSnapshot,
    contracts: &[CapabilityContract],
    object_resolution: &[ObjectResolutionResult],
    semantic_scores: &HashMap<String, f32>,
) -> Vec<EligibilityResult> {
    let resolution_by_node = object_resolution
        .iter()
        .map(|result| (result.node_id.as_str(), result))
        .collect::<HashMap<_, _>>();
    route_dag
        .nodes
        .iter()
        .map(|node| {
            let resolution = resolution_by_node.get(node.node_id.as_str()).copied();
            let mut eligible_actions = Vec::new();
            let mut rejected_actions = Vec::new();
            for contract in contracts {
                match contract_eligibility(node, contract, snapshot, resolution, semantic_scores) {
                    Ok(reason) => eligible_actions.push(EligibleAction {
                        action_id: contract.action_id.clone(),
                        contract_id: contract.contract_id.clone(),
                        confidence: semantic_scores
                            .get(&contract.action_id)
                            .copied()
                            .unwrap_or_default()
                            .clamp(0.0, 1.0),
                        proposed_inputs: proposed_inputs_for(node, contract, resolution),
                        eligibility_reason: reason,
                    }),
                    Err(reason) => rejected_actions.push(RejectedAction {
                        action_id: contract.action_id.clone(),
                        contract_id: contract.contract_id.clone(),
                        rejected_because: reason,
                    }),
                }
            }
            EligibilityResult {
                node_id: node.node_id.clone(),
                eligible_actions,
                rejected_actions,
                required_clarification: resolution
                    .filter(|result| result.requires_clarification)
                    .map(|_| "target object is ambiguous".to_string()),
                required_auth_action: None,
                required_setup_action: None,
                refusal: None,
            }
        })
        .collect()
}

fn contract_eligibility(
    node: &RouteIntentNode,
    contract: &CapabilityContract,
    snapshot: &RuntimeStateSnapshot,
    resolution: Option<&ObjectResolutionResult>,
    _semantic_scores: &HashMap<String, f32>,
) -> Result<String, String> {
    if !contract.supported_intents.contains(&node.intent_type) {
        return Err(format!(
            "contract does not support route intent {:?}",
            node.intent_type
        ));
    }
    if answer_route_lacks_tool_grounding(node, contract) {
        return Err(
            "answer-only route has no retrieval, source, or capability fact requiring this tool"
                .to_string(),
        );
    }
    if node.read_only && !contract.read_only {
        return Err("read-only route node cannot bind to a mutating contract".to_string());
    }
    if node.mutates_state && contract.read_only {
        return Err("mutating route node cannot bind to a read-only contract".to_string());
    }
    if node.destructive && !contract.destructive {
        return Err("destructive route node requires a destructive contract".to_string());
    }
    if !node.destructive && contract.destructive {
        return Err("non-destructive route node cannot bind to destructive contract".to_string());
    }
    if contract.requires_resolved_target
        && resolution
            .and_then(|result| result.target_object_id.as_ref())
            .is_none()
    {
        return Err("contract requires an exact resolved target".to_string());
    }
    if contract.requires_private_data
        && !private_data_access_available(snapshot, contract)
        && !contract.required_integrations.is_empty()
    {
        return Err("required private integration is not authenticated in snapshot".to_string());
    }
    if !schema_fillable_for_route(node, contract) {
        return Err("input schema is not fillable from route binding facts".to_string());
    }
    Ok(format!(
        "{} eligible because contract supports {:?}, side effects match, required state is available, and schema is fillable",
        contract.action_id, node.intent_type
    ))
}

fn answer_route_lacks_tool_grounding(
    node: &RouteIntentNode,
    contract: &CapabilityContract,
) -> bool {
    matches!(
        node.intent_type,
        IntentType::AnswerQuestion | IntentType::CasualChat
    ) && !matches!(
        contract.side_effect_level,
        ContractSideEffectLevel::AnswerOnly
    ) && node.required_data_source_hint.is_none()
        && node.required_notification_hint.is_none()
        && node.required_deployment_hint.is_none()
        && node.likely_required_capabilities.is_empty()
}

fn private_data_access_available(
    snapshot: &RuntimeStateSnapshot,
    contract: &CapabilityContract,
) -> bool {
    if contract.required_integrations.is_empty() {
        return true;
    }
    contract.required_integrations.iter().all(|integration| {
        snapshot.integrations.iter().any(|item| {
            item.integration_id == *integration
                && item.connected
                && item.auth_status == AuthStatus::Authenticated
        })
    })
}

fn schema_fillable_for_route(node: &RouteIntentNode, contract: &CapabilityContract) -> bool {
    if contract.open_world {
        return true;
    }
    let required = contract
        .input_schema
        .get("required")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if required.is_empty() {
        return true;
    }
    required.iter().all(|field| {
        matches!(
            *field,
            "query"
                | "message"
                | "prompt"
                | "topic"
                | "description"
                | "watcher_id"
                | "background_session_id"
                | "session_id"
                | "app_id"
        ) || (node.creates_new_object
            && matches!(
                *field,
                "files" | "source_dir" | "repo_url" | "items" | "poll_action" | "condition"
            ))
    })
}

fn proposed_inputs_for(
    node: &RouteIntentNode,
    contract: &CapabilityContract,
    resolution: Option<&ObjectResolutionResult>,
) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    let goal = first_non_empty([node.semantic_goal.as_str(), node.user_visible_goal.as_str()]);
    if schema_has_property(&contract.input_schema, "query") && !goal.is_empty() {
        map.insert(
            "query".to_string(),
            serde_json::Value::String(goal.to_string()),
        );
    }
    if schema_has_property(&contract.input_schema, "prompt") && !goal.is_empty() {
        map.insert(
            "prompt".to_string(),
            serde_json::Value::String(goal.to_string()),
        );
    }
    if schema_has_property(&contract.input_schema, "message") && !goal.is_empty() {
        map.insert(
            "message".to_string(),
            serde_json::Value::String(goal.to_string()),
        );
    }
    if let Some(target_id) = resolution.and_then(|result| result.target_object_id.as_ref()) {
        for field in [
            "watcher_id",
            "background_session_id",
            "session_id",
            "app_id",
            "artifact_id",
        ] {
            if schema_has_property(&contract.input_schema, field) {
                map.insert(
                    field.to_string(),
                    serde_json::Value::String(target_id.to_string()),
                );
            }
        }
    }
    serde_json::Value::Object(map)
}

fn bind_capabilities(
    route_dag: &AgentArkRouteDAG,
    eligibility: &[EligibilityResult],
    contracts: &[CapabilityContract],
    semantic_scores: &HashMap<String, f32>,
) -> Vec<CapabilityBinding> {
    let contract_by_id = contracts
        .iter()
        .map(|contract| (contract.contract_id.as_str(), contract))
        .collect::<HashMap<_, _>>();
    let node_by_id = route_dag
        .nodes
        .iter()
        .map(|node| (node.node_id.as_str(), node))
        .collect::<HashMap<_, _>>();
    eligibility
        .iter()
        .map(|result| {
            let Some(node) = node_by_id.get(result.node_id.as_str()).copied() else {
                return unsupported_binding(&result.node_id, "route node missing during binding");
            };
            if let Some(reason) = result.required_clarification.as_ref() {
                return CapabilityBinding {
                    node_id: result.node_id.clone(),
                    status: CapabilityBindingStatus::ClarificationNeeded,
                    action_id: None,
                    contract_id: None,
                    final_inputs: serde_json::json!({}),
                    missing_inputs: Vec::new(),
                    auth_issue: None,
                    setup_issue: None,
                    confirmation_reason: None,
                    refusal_reason: None,
                    binding_reason: reason.clone(),
                };
            }
            let mut eligible = result.eligible_actions.clone();
            eligible.sort_by(|left, right| {
                let left_contract = contract_by_id.get(left.contract_id.as_str()).copied();
                let right_contract = contract_by_id.get(right.contract_id.as_str()).copied();
                binding_rank(
                    node,
                    right_contract,
                    semantic_scores.get(&right.action_id).copied(),
                )
                .cmp(&binding_rank(
                    node,
                    left_contract,
                    semantic_scores.get(&left.action_id).copied(),
                ))
                .then_with(|| left.action_id.cmp(&right.action_id))
            });
            let Some(selected) = eligible.first() else {
                return unsupported_binding(
                    &result.node_id,
                    "no eligible capability contract remained after deterministic filtering",
                );
            };
            CapabilityBinding {
                node_id: result.node_id.clone(),
                status: if node.requires_confirmation {
                    CapabilityBindingStatus::ConfirmationNeeded
                } else {
                    CapabilityBindingStatus::Bound
                },
                action_id: Some(selected.action_id.clone()),
                contract_id: Some(selected.contract_id.clone()),
                final_inputs: selected.proposed_inputs.clone(),
                missing_inputs: Vec::new(),
                auth_issue: None,
                setup_issue: None,
                confirmation_reason: node
                    .requires_confirmation
                    .then(|| "policy requires confirmation before this mutation".to_string()),
                refusal_reason: None,
                binding_reason: format!(
                    "bound to eligible contract `{}`; picker considered only eligible actions",
                    selected.contract_id
                ),
            }
        })
        .collect()
}

fn binding_rank(
    node: &RouteIntentNode,
    contract: Option<&CapabilityContract>,
    semantic_score: Option<f32>,
) -> (u8, u8, u8, u8, u8, i32) {
    let Some(contract) = contract else {
        return (0, 0, 0, 0, 0, 0);
    };
    let exact_contract = u8::from(contract.supported_intents.contains(&node.intent_type));
    let object_continuity =
        u8::from(contract.requires_resolved_target == node.target_object_id.is_some());
    let connected = u8::from(!contract.requires_private_data);
    let lower_risk = match contract.side_effect_level {
        ContractSideEffectLevel::AnswerOnly => 5,
        ContractSideEffectLevel::ReadPublic => 4,
        ContractSideEffectLevel::ReadPrivate => 3,
        ContractSideEffectLevel::WriteLocal => 2,
        ContractSideEffectLevel::MutateSession => 2,
        ContractSideEffectLevel::DeployPublic => 1,
        ContractSideEffectLevel::SendExternal => 1,
        ContractSideEffectLevel::DeleteOrDestroy => 0,
    };
    let surface_fit = u8::from(
        contract
            .required_execution_surface
            .contains(&CurrentSurface::Chat),
    );
    let semantic = (semantic_score.unwrap_or_default().clamp(0.0, 1.0) * 1000.0).round() as i32;
    (
        exact_contract,
        object_continuity,
        connected,
        lower_risk,
        surface_fit,
        semantic,
    )
}

fn unsupported_binding(node_id: &str, reason: &str) -> CapabilityBinding {
    CapabilityBinding {
        node_id: node_id.to_string(),
        status: CapabilityBindingStatus::Unsupported,
        action_id: None,
        contract_id: None,
        final_inputs: serde_json::json!({}),
        missing_inputs: Vec::new(),
        auth_issue: None,
        setup_issue: None,
        confirmation_reason: None,
        refusal_reason: Some(reason.to_string()),
        binding_reason: reason.to_string(),
    }
}

fn run_policy_gate(
    route_dag: &AgentArkRouteDAG,
    bindings: &[CapabilityBinding],
    contracts: &[CapabilityContract],
    object_resolution: &[ObjectResolutionResult],
) -> Vec<PolicyDecision> {
    let node_by_id = route_dag
        .nodes
        .iter()
        .map(|node| (node.node_id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let contract_by_id = contracts
        .iter()
        .map(|contract| (contract.contract_id.as_str(), contract))
        .collect::<HashMap<_, _>>();
    let resolution_by_node = object_resolution
        .iter()
        .map(|result| (result.node_id.as_str(), result))
        .collect::<HashMap<_, _>>();
    bindings
        .iter()
        .map(|binding| {
            let node = node_by_id.get(binding.node_id.as_str()).copied();
            let contract = binding
                .contract_id
                .as_deref()
                .and_then(|id| contract_by_id.get(id).copied());
            let resolution = resolution_by_node.get(binding.node_id.as_str()).copied();
            if matches!(binding.status, CapabilityBindingStatus::Unsupported) {
                return PolicyDecision {
                    node_id: binding.node_id.clone(),
                    decision: PolicyDecisionKind::Deny,
                    reason: binding.binding_reason.clone(),
                };
            }
            if matches!(binding.status, CapabilityBindingStatus::ClarificationNeeded) {
                return PolicyDecision {
                    node_id: binding.node_id.clone(),
                    decision: PolicyDecisionKind::RequireClarification,
                    reason: binding.binding_reason.clone(),
                };
            }
            if let Some(node) = node {
                if node.destructive
                    && resolution
                        .and_then(|result| result.target_object_id.as_ref())
                        .is_none()
                {
                    return PolicyDecision {
                        node_id: binding.node_id.clone(),
                        decision: PolicyDecisionKind::RequireClarification,
                        reason: "destructive action requires exact target resolution".to_string(),
                    };
                }
                if node.destructive
                    || contract.is_some_and(|contract| contract.requires_confirmation)
                {
                    return PolicyDecision {
                        node_id: binding.node_id.clone(),
                        decision: PolicyDecisionKind::RequireConfirmation,
                        reason:
                            "policy requires confirmation before destructive or high-risk mutation"
                                .to_string(),
                    };
                }
                if node.read_only && contract.is_some_and(|contract| !contract.read_only) {
                    return PolicyDecision {
                        node_id: binding.node_id.clone(),
                        decision: PolicyDecisionKind::Deny,
                        reason: "policy denies read-only route binding to mutating action"
                            .to_string(),
                    };
                }
            }
            PolicyDecision {
                node_id: binding.node_id.clone(),
                decision: PolicyDecisionKind::Allow,
                reason: "policy allowed bound eligible capability".to_string(),
            }
        })
        .collect()
}

fn compile_execution_dag(
    route_dag: &AgentArkRouteDAG,
    bindings: &[CapabilityBinding],
    policy_decisions: &[PolicyDecision],
    contracts: &[CapabilityContract],
) -> ExecutionDAG {
    let node_by_id = route_dag
        .nodes
        .iter()
        .map(|node| (node.node_id.as_str(), node))
        .collect::<HashMap<_, _>>();
    let policy_by_node = policy_decisions
        .iter()
        .map(|policy| (policy.node_id.as_str(), policy))
        .collect::<HashMap<_, _>>();
    let contract_by_id = contracts
        .iter()
        .map(|contract| (contract.contract_id.as_str(), contract))
        .collect::<HashMap<_, _>>();
    let mut nodes = Vec::new();
    for binding in bindings {
        let Some(route_node) = node_by_id.get(binding.node_id.as_str()).copied() else {
            continue;
        };
        let policy = policy_by_node.get(binding.node_id.as_str()).copied();
        let status = match policy.map(|decision| &decision.decision) {
            Some(PolicyDecisionKind::Allow) if binding.status == CapabilityBindingStatus::Bound => {
                ExecutionNodeStatus::Ready
            }
            Some(PolicyDecisionKind::RequireAuth) => ExecutionNodeStatus::WaitingForAuth,
            Some(PolicyDecisionKind::RequireConfirmation) => {
                ExecutionNodeStatus::WaitingForConfirmation
            }
            Some(PolicyDecisionKind::RequireClarification) => {
                ExecutionNodeStatus::WaitingForClarification
            }
            Some(PolicyDecisionKind::Deny) => ExecutionNodeStatus::Skipped,
            Some(PolicyDecisionKind::RequireSafeFallback) => ExecutionNodeStatus::Skipped,
            _ => match binding.status {
                CapabilityBindingStatus::Bound => ExecutionNodeStatus::Ready,
                CapabilityBindingStatus::AuthNeeded => ExecutionNodeStatus::WaitingForAuth,
                CapabilityBindingStatus::ConfirmationNeeded => {
                    ExecutionNodeStatus::WaitingForConfirmation
                }
                CapabilityBindingStatus::ClarificationNeeded => {
                    ExecutionNodeStatus::WaitingForClarification
                }
                _ => ExecutionNodeStatus::Skipped,
            },
        };
        let contract = binding
            .contract_id
            .as_deref()
            .and_then(|id| contract_by_id.get(id).copied());
        nodes.push(ExecutionNode {
            node_id: format!("exec:{}", binding.node_id),
            route_node_id: binding.node_id.clone(),
            action_id: binding.action_id.clone(),
            status,
            inputs: binding.final_inputs.clone(),
            expected_output: Some(route_node.user_visible_goal.clone()),
            read_only: route_node.read_only,
            mutates_state: route_node.mutates_state,
            destructive: route_node.destructive,
            approval_required: policy.is_some_and(|decision| {
                decision.decision == PolicyDecisionKind::RequireConfirmation
            }),
            retry_policy: contract
                .filter(|contract| contract.retry_safe)
                .map(|_| "retry replay-safe read or idempotent action only".to_string()),
            fallback_node_ids: Vec::new(),
            compensation_action: contract.and_then(|contract| contract.compensation_action.clone()),
            checkpoint_policy: "checkpoint_after_node".to_string(),
        });
    }
    let execution_order = topological_route_order(route_dag)
        .into_iter()
        .map(|node_id| format!("exec:{node_id}"))
        .collect::<Vec<_>>();
    let requires_user_input_before_execution = nodes.iter().any(|node| {
        matches!(
            node.status,
            ExecutionNodeStatus::WaitingForAuth
                | ExecutionNodeStatus::WaitingForClarification
                | ExecutionNodeStatus::WaitingForConfirmation
                | ExecutionNodeStatus::WaitingForExternalLogin
        )
    });
    let parallel_groups = parallel_groups(route_dag)
        .into_iter()
        .map(|group| {
            group
                .into_iter()
                .map(|node_id| format!("exec:{node_id}"))
                .collect()
        })
        .collect();
    let final_response_strategy = if requires_user_input_before_execution {
        "explain_waiting_gate_with_capability_facts".to_string()
    } else if nodes
        .iter()
        .any(|node| node.status == ExecutionNodeStatus::Ready)
    {
        "execute_ready_nodes_then_summarize_outputs".to_string()
    } else {
        "answer_without_tool_or_explain_unsupported".to_string()
    };
    ExecutionDAG {
        schema_version: ROUTER_SCHEMA_VERSION,
        execution_dag_id: format!("execution:{}", route_dag.route_dag_id),
        route_dag_id: route_dag.route_dag_id.clone(),
        nodes,
        edges: route_dag.edges.clone(),
        execution_order,
        parallel_groups,
        requires_user_input_before_execution,
        final_response_strategy,
    }
}

fn topological_route_order(route_dag: &AgentArkRouteDAG) -> Vec<String> {
    let ids = route_dag
        .nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<BTreeSet<_>>();
    let mut incoming = ids
        .iter()
        .map(|id| (id.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<String, Vec<String>>::new();
    for edge in &route_dag.edges {
        if ids.contains(&edge.from_node_id) && ids.contains(&edge.to_node_id) {
            *incoming.entry(edge.to_node_id.clone()).or_default() += 1;
            outgoing
                .entry(edge.from_node_id.clone())
                .or_default()
                .push(edge.to_node_id.clone());
        }
    }
    let mut ready = incoming
        .iter()
        .filter_map(|(id, count)| (*count == 0).then(|| id.clone()))
        .collect::<VecDeque<_>>();
    let mut ordered = Vec::new();
    while let Some(id) = ready.pop_front() {
        ordered.push(id.clone());
        for child in outgoing.get(&id).cloned().unwrap_or_default() {
            if let Some(count) = incoming.get_mut(&child) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    ready.push_back(child);
                }
            }
        }
    }
    if ordered.len() == ids.len() {
        ordered
    } else {
        route_dag
            .nodes
            .iter()
            .map(|node| node.node_id.clone())
            .collect()
    }
}

fn parallel_groups(route_dag: &AgentArkRouteDAG) -> Vec<Vec<String>> {
    let mut depths = BTreeMap::<String, usize>::new();
    for id in topological_route_order(route_dag) {
        let depth = route_dag
            .edges
            .iter()
            .filter(|edge| edge.to_node_id == id)
            .filter_map(|edge| depths.get(&edge.from_node_id).copied())
            .max()
            .map(|value| value + 1)
            .unwrap_or(0);
        depths.insert(id, depth);
    }
    let mut grouped = BTreeMap::<usize, Vec<String>>::new();
    for (node_id, depth) in depths {
        grouped.entry(depth).or_default().push(node_id);
    }
    grouped.into_values().collect()
}

fn supported_intents_for_action(
    action: &ActionDef,
    metadata: &crate::actions::ActionPlannerMetadata,
) -> Vec<IntentType> {
    let name = action.name.as_str();
    match name {
        "ark_inspect" => vec![
            IntentType::CurrentStatusQuery,
            IntentType::InternalSystemQuery,
            IntentType::ArkPulseQuery,
            IntentType::ArkSentinelQuery,
            IntentType::TraceQuery,
        ],
        "list_watchers" => vec![IntentType::CurrentStatusQuery],
        "watch" => vec![IntentType::WatcherCreate, IntentType::WatcherUpdate],
        "watcher_delete" => vec![IntentType::WatcherDelete],
        "schedule_task" => vec![IntentType::ReminderCreate],
        "app_deploy" => vec![
            IntentType::AppCreate,
            IntentType::AppEdit,
            IntentType::AppDeploy,
        ],
        "app_restart" | "app_stop" => vec![IntentType::AppEdit],
        "app_delete" => vec![IntentType::AppEdit],
        "browser_auto" => vec![IntentType::BrowserAutomation],
        "file_read" => vec![
            IntentType::MemoryRecall,
            IntentType::CurrentStatusQuery,
            IntentType::FileOperation,
        ],
        "file_write" => vec![IntentType::FileOperation, IntentType::CanvasOperation],
        "web_search" | "research" | "page_fetch" => {
            vec![IntentType::PublicWebResearch, IntentType::DeepResearch]
        }
        "gmail_scan" | "calendar_today" | "calendar_list" | "calendar_free" => {
            vec![IntentType::PrivateIntegrationQuery]
        }
        "calendar_create" => vec![IntentType::CalendarEventCreate],
        "gmail_reply" | "notify_user" => vec![IntentType::NotificationChannelUpdate],
        "capability_resolve" | "list_integrations" => vec![
            IntentType::CapabilityExplanation,
            IntentType::IntegrationInstall,
            IntentType::IntegrationAuthReconnect,
        ],
        _ if metadata.role == PlannerActionRole::Inspection
            || metadata.role == PlannerActionRole::DataSource =>
        {
            let mut intents = vec![IntentType::AnswerQuestion, IntentType::CurrentStatusQuery];
            if metadata.requires_auth
                || action.authorization.requires_auth
                || !action.authorization.access.integration_ids.is_empty()
            {
                intents.push(IntentType::PrivateIntegrationQuery);
                intents.push(IntentType::InternalSystemQuery);
            }
            intents
        }
        _ if metadata.role == PlannerActionRole::Mutation => {
            vec![IntentType::FileOperation]
        }
        _ => vec![IntentType::AnswerQuestion],
    }
}

fn contract_side_effect(
    action: &ActionDef,
    metadata: &crate::actions::ActionPlannerMetadata,
) -> ContractSideEffectLevel {
    if action.name == "app_deploy" {
        return ContractSideEffectLevel::WriteLocal;
    }
    if action.name == "app_delete" || action.name == "watcher_delete" {
        return ContractSideEffectLevel::DeleteOrDestroy;
    }
    match metadata.side_effect_level {
        PlannerSideEffectLevel::None
            if metadata.requires_auth || action.authorization.requires_auth =>
        {
            ContractSideEffectLevel::ReadPrivate
        }
        PlannerSideEffectLevel::None => ContractSideEffectLevel::ReadPublic,
        PlannerSideEffectLevel::Notify => ContractSideEffectLevel::SendExternal,
        PlannerSideEffectLevel::Write
            if metadata.integration_class == PlannerIntegrationClass::App =>
        {
            ContractSideEffectLevel::WriteLocal
        }
        PlannerSideEffectLevel::Write => ContractSideEffectLevel::MutateSession,
    }
}

fn contract_durability(
    action: &ActionDef,
    metadata: &crate::actions::ActionPlannerMetadata,
) -> ContractDurability {
    match action.name.as_str() {
        "app_deploy" | "app_restart" | "app_stop" | "app_delete" => ContractDurability::DeployedApp,
        "watch" | "watcher_delete" => ContractDurability::RecurringWatcher,
        "schedule_task" | "notify_user" => ContractDurability::Reminder,
        "calendar_create" => ContractDurability::CalendarEvent,
        "file_write" | "file_read" => ContractDurability::LocalArtifact,
        _ if metadata.delivery_mode == PlannerDeliveryMode::Conditional => {
            ContractDurability::RecurringWatcher
        }
        _ if metadata.delivery_mode == PlannerDeliveryMode::Async => ContractDurability::Reminder,
        _ => ContractDurability::OneShot,
    }
}

fn required_state_for_action(action: &ActionDef) -> Vec<String> {
    if action_requires_resolved_target(action) {
        vec!["resolved_target".to_string()]
    } else {
        Vec::new()
    }
}

fn action_requires_resolved_target(action: &ActionDef) -> bool {
    matches!(
        action.name.as_str(),
        "watcher_delete" | "app_restart" | "app_stop" | "app_delete"
    )
}

fn action_accepts_open_world_inputs(action: &ActionDef) -> bool {
    matches!(
        action.name.as_str(),
        "app_deploy" | "watch" | "schedule_task" | "browser_auto" | "research" | "file_write"
    )
}

fn compensation_action_for_action(action: &ActionDef) -> Option<String> {
    match action.name.as_str() {
        "app_deploy" => Some("app_stop".to_string()),
        "watch" => Some("watcher_delete".to_string()),
        _ => None,
    }
}

fn schema_has_property(schema: &serde_json::Value, field: &str) -> bool {
    schema
        .get("properties")
        .and_then(|value| value.as_object())
        .is_some_and(|properties| properties.contains_key(field))
}

fn first_non_empty<'a, I>(items: I) -> &'a str
where
    I: IntoIterator<Item = &'a str>,
{
    items
        .into_iter()
        .map(str::trim)
        .find(|value| !value.is_empty())
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(name: &str, schema: serde_json::Value, capabilities: &[&str]) -> ActionDef {
        ActionDef {
            name: name.to_string(),
            description: name.to_string(),
            input_schema: schema,
            capabilities: capabilities.iter().map(|value| value.to_string()).collect(),
            ..ActionDef::default()
        }
    }

    fn routing_goal(
        id: &str,
        durability: &str,
        side_effect: &str,
        groundings: &[&str],
    ) -> InboundTurnGoal {
        InboundTurnGoal {
            id: id.to_string(),
            intent_summary: format!("{id} goal"),
            capability_query: format!("{id} capability"),
            expected_outcome: format!("{id} outcome"),
            durability: durability.to_string(),
            groundings: groundings.iter().map(|value| value.to_string()).collect(),
            side_effect: side_effect.to_string(),
            dependencies: Vec::new(),
        }
    }

    fn trace_for(goals: Vec<InboundTurnGoal>, actions: Vec<ActionDef>) -> RouteTrace {
        let routing = InboundRoutingSignal {
            current_answer_expected: true,
            goals,
            ..Default::default()
        };
        let envelope = UserMessageEnvelope::chat("test request", Some("c1"), "u1");
        let snapshot = RuntimeStateSnapshot::from_actions(&actions);
        route_user_message(
            envelope,
            snapshot,
            Some(&routing),
            &actions,
            &HashMap::new(),
        )
    }

    #[test]
    fn semantic_router_mixed_app_deploy_and_internal_query_preserves_nodes_and_edges() {
        let trace = trace_for(
            vec![
                routing_goal("g1", "deployment", "write", &[]),
                routing_goal("g2", "none", "none", &["local_state"]),
            ],
            vec![
                action(
                    "app_deploy",
                    serde_json::json!({"type":"object"}),
                    &["app_hosting"],
                ),
                action(
                    "ark_inspect",
                    serde_json::json!({"type":"object"}),
                    &["platform_observability"],
                ),
            ],
        );

        let intents = trace
            .route_dag
            .nodes
            .iter()
            .map(|node| node.intent_type.clone())
            .collect::<Vec<_>>();
        assert!(intents.contains(&IntentType::AppCreate));
        assert!(intents.contains(&IntentType::AppDeploy));
        assert!(intents.contains(&IntentType::InternalSystemQuery));
        assert!(trace.route_dag.edges.iter().any(|edge| {
            trace
                .route_dag
                .nodes
                .iter()
                .find(|node| node.node_id == edge.from_node_id)
                .is_some_and(|node| node.intent_type == IntentType::AppCreate)
                && trace
                    .route_dag
                    .nodes
                    .iter()
                    .find(|node| node.node_id == edge.to_node_id)
                    .is_some_and(|node| node.intent_type == IntentType::AppDeploy)
        }));
    }

    #[test]
    fn semantic_router_read_only_node_never_binds_to_mutation() {
        let trace = trace_for(
            vec![routing_goal("g1", "none", "none", &["local_state"])],
            vec![
                action(
                    "ark_inspect",
                    serde_json::json!({"type":"object"}),
                    &["platform_observability"],
                ),
                action(
                    "app_deploy",
                    serde_json::json!({"type":"object"}),
                    &["app_hosting"],
                ),
            ],
        );
        let eligibility = &trace.eligibility[0];
        assert!(
            eligibility
                .eligible_actions
                .iter()
                .all(|item| item.action_id != "app_deploy")
        );
        assert!(
            eligibility
                .rejected_actions
                .iter()
                .any(|item| { item.action_id == "app_deploy" })
        );
        assert!(!trace.execution_dag.has_ready_action("app_deploy"));
    }

    #[test]
    fn semantic_router_answer_only_default_does_not_bind_inspection() {
        let actions = vec![
            action(
                "ark_inspect",
                serde_json::json!({"type":"object"}),
                &["platform_observability"],
            ),
            action("generic_lookup", serde_json::json!({"type":"object"}), &[]),
        ];
        let envelope = UserMessageEnvelope::chat("test request", Some("c1"), "u1");
        let snapshot = RuntimeStateSnapshot::from_actions(&actions);
        let trace = route_user_message(envelope, snapshot, None, &actions, &HashMap::new());

        assert_eq!(
            trace.route_dag.nodes[0].intent_type,
            IntentType::AnswerQuestion
        );
        assert!(
            trace
                .eligibility
                .iter()
                .flat_map(|result| result.rejected_actions.iter())
                .any(|item| item.action_id == "ark_inspect")
        );
        assert!(
            trace
                .eligibility
                .iter()
                .flat_map(|result| result.rejected_actions.iter())
                .any(|item| item.action_id == "generic_lookup"
                    && item
                        .rejected_because
                        .contains("no retrieval, source, or capability fact"))
        );
        assert!(!trace.execution_dag.has_ready_action("ark_inspect"));
        assert!(!trace.execution_dag.has_ready_action("generic_lookup"));
    }

    #[test]
    fn semantic_router_rejected_actions_do_not_reach_execution_dag() {
        let trace = trace_for(
            vec![routing_goal("g1", "none", "none", &["external_info"])],
            vec![
                action(
                    "web_search",
                    serde_json::json!({"type":"object","properties":{"query":{"type":"string"}}}),
                    &["search"],
                ),
                action("watch", serde_json::json!({"type":"object"}), &["watcher"]),
            ],
        );
        assert!(!trace.execution_dag.has_ready_action("watch"));
        assert!(trace.execution_dag.has_ready_action("web_search"));
    }

    #[test]
    fn semantic_router_durable_monitor_grounding_still_binds_watcher() {
        let trace = trace_for(
            vec![routing_goal(
                "g1",
                "recurring_monitor",
                "notify",
                &["local_state"],
            )],
            vec![
                action("watch", serde_json::json!({"type":"object"}), &["watcher"]),
                action(
                    "ark_inspect",
                    serde_json::json!({"type":"object"}),
                    &["platform_observability"],
                ),
            ],
        );

        assert_eq!(
            trace.route_dag.nodes[0].intent_type,
            IntentType::WatcherCreate
        );
        assert!(trace.execution_dag.has_ready_action("watch"));
        assert!(!trace.execution_dag.has_ready_action("ark_inspect"));
    }

    #[test]
    fn semantic_router_mutating_route_never_binds_read_only_contract() {
        let node = RouteIntentNode {
            node_id: "g1".to_string(),
            intent_type: IntentType::CurrentStatusQuery,
            user_visible_goal: "Create or update durable work".to_string(),
            semantic_goal: "Create or update durable work".to_string(),
            read_only: false,
            mutates_state: true,
            destructive: false,
            requires_confirmation: false,
            temporal_scope: None,
            target_object_hint: None,
            target_object_type: None,
            target_object_id: None,
            creates_new_object: true,
            likely_required_capabilities: vec!["durable work".to_string()],
            required_data_source_hint: None,
            required_notification_hint: None,
            required_deployment_hint: None,
            missing_inputs: Vec::new(),
            safety_flags: Vec::new(),
            confidence: 0.8,
            reason: "test".to_string(),
            decomposition_method: "test".to_string(),
            preconditions: Vec::new(),
            effects: Vec::new(),
            partial_order_constraints: Vec::new(),
        };
        let contract = capability_contract_for_action(&action(
            "ark_inspect",
            serde_json::json!({"type":"object"}),
            &["platform_observability"],
        ));
        let snapshot = RuntimeStateSnapshot::from_actions(&[]);

        let error = contract_eligibility(&node, &contract, &snapshot, None, &HashMap::new())
            .expect_err("read-only contract must not satisfy mutating route");

        assert!(error.contains("mutating route node cannot bind to a read-only contract"));
    }

    #[test]
    fn semantic_router_read_only_ready_node_does_not_make_mixed_route_read_only() {
        let trace = trace_for(
            vec![
                routing_goal("g1", "recurring_monitor", "notify", &[]),
                routing_goal("g2", "none", "none", &["local_state"]),
            ],
            vec![action(
                "ark_inspect",
                serde_json::json!({"type":"object"}),
                &["platform_observability"],
            )],
        );

        assert!(trace.execution_dag.ready_nodes_all_read_only());
        assert!(!trace.route_dag.all_nodes_read_only());
        assert!(
            trace.route_dag.nodes.iter().any(|node| {
                node.intent_type == IntentType::WatcherCreate && node.mutates_state
            })
        );
    }

    #[test]
    fn semantic_router_high_score_rejected_action_cannot_win_picker() {
        let mut scores = HashMap::new();
        scores.insert("app_deploy".to_string(), 1.0);
        scores.insert("ark_inspect".to_string(), 0.1);
        let routing = InboundRoutingSignal {
            current_answer_expected: true,
            goals: vec![routing_goal("g1", "none", "none", &["local_state"])],
            ..Default::default()
        };
        let actions = vec![
            action(
                "ark_inspect",
                serde_json::json!({"type":"object"}),
                &["platform_observability"],
            ),
            action(
                "app_deploy",
                serde_json::json!({"type":"object"}),
                &["app_hosting"],
            ),
        ];
        let envelope = UserMessageEnvelope::chat("test request", Some("c1"), "u1");
        let snapshot = RuntimeStateSnapshot::from_actions(&actions);
        let trace = route_user_message(envelope, snapshot, Some(&routing), &actions, &scores);

        assert!(
            trace
                .eligibility
                .iter()
                .flat_map(|result| result.rejected_actions.iter())
                .any(|item| item.action_id == "app_deploy")
        );
        assert!(!trace.execution_dag.has_ready_action("app_deploy"));
        assert!(trace.execution_dag.has_ready_action("ark_inspect"));
    }

    #[test]
    fn semantic_router_mixed_capability_and_live_state_acts_on_live_state() {
        let trace = trace_for(
            vec![routing_goal(
                "g1",
                "none",
                "none",
                &["agentark_capabilities", "local_state"],
            )],
            vec![
                action(
                    "agentark_capability_lookup",
                    serde_json::json!({"type":"object","properties":{"query":{"type":"string"}}}),
                    &["agentark_capabilities", "capability_inventory"],
                ),
                action(
                    "ark_inspect",
                    serde_json::json!({"type":"object"}),
                    &["platform_observability"],
                ),
            ],
        );

        assert_eq!(
            trace.route_dag.nodes[0].intent_type,
            IntentType::InternalSystemQuery
        );
        assert!(trace.execution_dag.has_ready_action("ark_inspect"));
        assert!(
            !trace
                .execution_dag
                .has_ready_action("agentark_capability_lookup")
        );
    }

    #[test]
    fn semantic_router_authorized_connected_read_action_can_bind_private_query() {
        let mut read_action = action(
            "connected_service_scan",
            serde_json::json!({"type":"object","properties":{"query":{"type":"string"}}}),
            &["custom_api", "records"],
        );
        read_action.authorization.requires_auth = true;
        read_action.authorization.access.integration_ids = vec!["connected_service".to_string()];

        let trace = trace_for(
            vec![routing_goal("g1", "none", "none", &["local_state"])],
            vec![read_action],
        );

        assert!(
            trace
                .eligibility
                .iter()
                .flat_map(|result| result.eligible_actions.iter())
                .any(|item| item.action_id == "connected_service_scan")
        );
        assert!(
            trace
                .execution_dag
                .has_ready_action("connected_service_scan")
        );
    }

    #[test]
    fn semantic_router_unfillable_schema_is_rejected_before_binding() {
        let trace = trace_for(
            vec![routing_goal("g1", "none", "none", &["external_info"])],
            vec![
                action(
                    "web_search",
                    serde_json::json!({
                        "type":"object",
                        "properties":{"opaque":{"type":"string"}},
                        "required":["opaque"]
                    }),
                    &["search"],
                ),
                action(
                    "page_fetch",
                    serde_json::json!({
                        "type":"object",
                        "properties":{"query":{"type":"string"}},
                        "required":["query"]
                    }),
                    &["search"],
                ),
            ],
        );

        assert!(
            trace
                .eligibility
                .iter()
                .flat_map(|result| result.rejected_actions.iter())
                .any(|item| item.action_id == "web_search"
                    && item.rejected_because.contains("schema is not fillable"))
        );
        assert!(!trace.execution_dag.has_ready_action("web_search"));
        assert!(trace.execution_dag.has_ready_action("page_fetch"));
    }

    #[test]
    fn semantic_router_replay_is_deterministic_for_same_envelope_and_snapshot() {
        let actions = vec![
            action(
                "app_deploy",
                serde_json::json!({"type":"object"}),
                &["app_hosting"],
            ),
            action(
                "ark_inspect",
                serde_json::json!({"type":"object"}),
                &["platform_observability"],
            ),
        ];
        let routing = InboundRoutingSignal {
            current_answer_expected: true,
            goals: vec![
                routing_goal("g1", "deployment", "write", &[]),
                routing_goal("g2", "none", "none", &["local_state"]),
            ],
            ..Default::default()
        };
        let mut envelope = UserMessageEnvelope::chat("test request", Some("c1"), "u1");
        envelope.message_id = "message-1".to_string();
        let snapshot = RuntimeStateSnapshot::from_actions(&actions);

        let first = route_user_message(
            envelope.clone(),
            snapshot.clone(),
            Some(&routing),
            &actions,
            &HashMap::new(),
        );
        let second = route_user_message(
            envelope,
            snapshot,
            Some(&routing),
            &actions,
            &HashMap::new(),
        );

        assert_eq!(first.route_dag, second.route_dag);
        assert_eq!(first.eligibility, second.eligibility);
        assert_eq!(first.capability_bindings, second.capability_bindings);
        assert_eq!(first.policy_decisions, second.policy_decisions);
        assert_eq!(first.execution_dag, second.execution_dag);
    }

    #[test]
    fn semantic_router_destructive_ambiguous_target_waits_for_clarification() {
        let mut actions = vec![action(
            "watcher_delete",
            serde_json::json!({
                "type": "object",
                "properties": {"watcher_id": {"type": "string"}},
                "required": ["watcher_id"]
            }),
            &["watcher"],
        )];
        actions[0].authorization.human_approval.required = true;
        let routing = InboundRoutingSignal {
            current_answer_expected: true,
            goals: vec![routing_goal("g1", "recurring_monitor", "delete", &[])],
            ..Default::default()
        };
        let envelope = UserMessageEnvelope::chat("test delete", Some("c1"), "u1");
        let mut snapshot = RuntimeStateSnapshot::from_actions(&actions);
        snapshot.add_runtime_object(RuntimeObjectSnapshot {
            object_type: RuntimeObjectType::Watcher,
            object_id: "w1".to_string(),
            name: "one".to_string(),
            status: "active".to_string(),
            ..Default::default()
        });
        snapshot.add_runtime_object(RuntimeObjectSnapshot {
            object_type: RuntimeObjectType::Watcher,
            object_id: "w2".to_string(),
            name: "two".to_string(),
            status: "active".to_string(),
            ..Default::default()
        });
        let trace = route_user_message(
            envelope,
            snapshot,
            Some(&routing),
            &actions,
            &HashMap::new(),
        );
        assert!(
            trace
                .object_resolution
                .iter()
                .any(|result| result.requires_clarification)
        );
        assert!(trace.execution_dag.requires_user_input_before_execution);
        assert!(!trace.execution_dag.has_ready_action("watcher_delete"));
    }
}
