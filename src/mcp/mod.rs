//! Model Context Protocol (MCP) support
//! Exposes agent tools and resources via JSON-RPC over HTTP

use serde::{Deserialize, Serialize};

pub mod client;
pub mod registry;

/// MCP tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// MCP resource definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: String,
    pub mime_type: String,
}

/// MCP JSON-RPC request
#[derive(Debug, Deserialize)]
pub struct McpRequest {
    pub _jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// MCP JSON-RPC response
#[derive(Debug, Serialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

#[derive(Debug, Serialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
}

/// MCP server that exposes agent capabilities
pub struct McpServer {
    tools: Vec<McpTool>,
    resources: Vec<McpResource>,
    /// Optional token for defense-in-depth authentication
    _expected_token: Option<String>,
}

impl McpServer {
    pub fn new() -> Self {
        let tools = vec![
            McpTool {
                name: "chat".to_string(),
                description: "Send a message to the AI agent and get a response".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string", "description": "The message to send" },
                        "channel": { "type": "string", "description": "Channel identifier", "default": "mcp" },
                        "conversation_id": { "type": "string", "description": "Optional conversation id for multi-turn context" },
                        "project_id": { "type": "string", "description": "Optional project id" }
                    },
                    "required": ["message"]
                }),
            },
            McpTool {
                name: "memory_search".to_string(),
                description: "Search the agent's memory for relevant information".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" },
                        "limit": { "type": "integer", "description": "Max results", "default": 5 }
                    },
                    "required": ["query"]
                }),
            },
            McpTool {
                name: "document_search".to_string(),
                description: "Search uploaded documents for relevant content".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" },
                        "limit": { "type": "integer", "description": "Max results", "default": 5 }
                    },
                    "required": ["query"]
                }),
            },
            McpTool {
                name: "list_actions".to_string(),
                description: "List available agent actions/tools".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            McpTool {
                name: "execute_action".to_string(),
                description: "Execute a named action with arguments".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "description": "Action name" },
                        "arguments": { "type": "object", "description": "Action arguments" }
                    },
                    "required": ["action"]
                }),
            },
        ];

        let resources = vec![
            McpResource {
                uri: "agentark://status".to_string(),
                name: "Agent Status".to_string(),
                description: "Current agent status and statistics".to_string(),
                mime_type: "application/json".to_string(),
            },
            McpResource {
                uri: "agentark://memory".to_string(),
                name: "Memory Stats".to_string(),
                description: "Memory system statistics".to_string(),
                mime_type: "application/json".to_string(),
            },
        ];

        Self {
            tools,
            resources,
            _expected_token: None,
        }
    }

    /// Handle an MCP JSON-RPC request
    pub fn handle_request(&self, request: &McpRequest) -> McpResponse {
        let result = match request.method.as_str() {
            "initialize" => Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": { "listChanged": false },
                    "resources": { "subscribe": false, "listChanged": false }
                },
                "serverInfo": {
                    "name": "agentark",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
            "tools/list" => Some(serde_json::json!({
                "tools": self.tools
            })),
            "resources/list" => Some(serde_json::json!({
                "resources": self.resources
            })),
            "ping" => Some(serde_json::json!({})),
            _ => {
                return McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    result: None,
                    error: Some(McpError {
                        code: -32601,
                        message: format!("Method not found: {}", request.method),
                    }),
                };
            }
        };

        McpResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id.clone(),
            result,
            error: None,
        }
    }
}
