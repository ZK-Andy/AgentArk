//! LLM client for agent reasoning

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// Supported LLM providers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "lowercase")]
pub enum LlmProvider {
    Anthropic {
        api_key: String,
        model: String,
    },
    OpenAI {
        api_key: String,
        model: String,
        base_url: Option<String>,
    },
    Ollama {
        base_url: String,
        model: String,
    },
}

impl Default for LlmProvider {
    fn default() -> Self {
        Self::Ollama {
            base_url: "http://localhost:11434".to_string(),
            model: "llama3.2".to_string(),
        }
    }
}

/// Message role
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// Chat message
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// Tool call from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// LLM response
#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

/// LLM client
#[derive(Clone)]
pub struct LlmClient {
    provider: LlmProvider,
    client: reqwest::Client,
}

impl LlmClient {
    pub fn new(provider: &LlmProvider) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()?;

        Ok(Self {
            provider: provider.clone(),
            client,
        })
    }

    /// Send a chat request to the LLM
    pub async fn chat(
        &self,
        system_prompt: &str,
        user_message: &str,
        _memories: &[crate::memory::MemoryEntry],
        actions: &[crate::actions::ActionDef],
    ) -> Result<LlmResponse> {
        // Call with empty history for backwards compatibility
        self.chat_with_history(system_prompt, user_message, &[], _memories, actions).await
    }

    /// Send a chat request with conversation history
    pub async fn chat_with_history(
        &self,
        system_prompt: &str,
        user_message: &str,
        history: &[crate::core::agent::ConversationMessage],
        _memories: &[crate::memory::MemoryEntry],
        actions: &[crate::actions::ActionDef],
    ) -> Result<LlmResponse> {
        match &self.provider {
            LlmProvider::Anthropic { api_key, model } => {
                self.chat_anthropic_with_history(api_key, model, system_prompt, user_message, history, actions)
                    .await
            }
            LlmProvider::OpenAI {
                api_key,
                model,
                base_url,
            } => {
                self.chat_openai_with_history(
                    api_key,
                    model,
                    base_url.as_deref(),
                    system_prompt,
                    user_message,
                    history,
                    actions,
                )
                .await
            }
            LlmProvider::Ollama { base_url, model } => {
                self.chat_ollama_with_history(base_url, model, system_prompt, user_message, history)
                    .await
            }
        }
    }

    async fn chat_anthropic_with_history(
        &self,
        api_key: &str,
        model: &str,
        system_prompt: &str,
        user_message: &str,
        history: &[crate::core::agent::ConversationMessage],
        actions: &[crate::actions::ActionDef],
    ) -> Result<LlmResponse> {
        #[derive(Serialize)]
        struct AnthropicRequest {
            model: String,
            max_tokens: u32,
            system: String,
            messages: Vec<AnthropicMessage>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            tools: Vec<AnthropicTool>,
        }

        #[derive(Serialize)]
        struct AnthropicMessage {
            role: String,
            content: String,
        }

        #[derive(Serialize)]
        struct AnthropicTool {
            name: String,
            description: String,
            input_schema: serde_json::Value,
        }

        #[derive(Deserialize)]
        struct AnthropicResponse {
            content: Vec<ContentBlock>,
        }

        #[derive(Deserialize)]
        #[serde(tag = "type")]
        enum ContentBlock {
            #[serde(rename = "text")]
            Text { text: String },
            #[serde(rename = "tool_use")]
            ToolUse {
                id: String,
                name: String,
                input: serde_json::Value,
            },
        }

        let tools: Vec<AnthropicTool> = actions
            .iter()
            .map(|s| AnthropicTool {
                name: s.name.clone(),
                description: s.description.clone(),
                input_schema: s.input_schema.clone(),
            })
            .collect();

        // Build messages array with history (exclude the last user message as we add it separately)
        let mut messages: Vec<AnthropicMessage> = history
            .iter()
            .filter(|m| !(m.role == "user" && m.content == user_message))
            .map(|m| AnthropicMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();

        // Add the current user message
        messages.push(AnthropicMessage {
            role: "user".to_string(),
            content: user_message.to_string(),
        });

        let request = AnthropicRequest {
            model: model.to_string(),
            max_tokens: 4096,
            system: system_prompt.to_string(),
            messages,
            tools,
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(anyhow!("Anthropic API error: {}", error));
        }

        let response: AnthropicResponse = response.json().await?;

        let mut content = String::new();
        let mut tool_calls = Vec::new();

        for block in response.content {
            match block {
                ContentBlock::Text { text } => {
                    content.push_str(&text);
                }
                ContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ToolCall {
                        id,
                        name,
                        arguments: input,
                    });
                }
            }
        }

        Ok(LlmResponse {
            content,
            tool_calls,
        })
    }

    async fn chat_openai_with_history(
        &self,
        api_key: &str,
        model: &str,
        base_url: Option<&str>,
        system_prompt: &str,
        user_message: &str,
        history: &[crate::core::agent::ConversationMessage],
        actions: &[crate::actions::ActionDef],
    ) -> Result<LlmResponse> {
        #[derive(Serialize)]
        struct OpenAIRequest {
            model: String,
            messages: Vec<OpenAIMessage>,
            #[serde(skip_serializing_if = "Vec::is_empty")]
            tools: Vec<OpenAITool>,
        }

        #[derive(Serialize)]
        struct OpenAIMessage {
            role: String,
            content: String,
        }

        #[derive(Serialize)]
        struct OpenAITool {
            #[serde(rename = "type")]
            tool_type: String,
            function: OpenAIFunction,
        }

        #[derive(Serialize)]
        struct OpenAIFunction {
            name: String,
            description: String,
            parameters: serde_json::Value,
        }

        #[derive(Deserialize)]
        struct OpenAIResponse {
            choices: Vec<OpenAIChoice>,
        }

        #[derive(Deserialize)]
        struct OpenAIChoice {
            message: OpenAIResponseMessage,
        }

        #[derive(Deserialize)]
        struct OpenAIResponseMessage {
            content: Option<String>,
            tool_calls: Option<Vec<OpenAIToolCall>>,
        }

        #[derive(Deserialize)]
        struct OpenAIToolCall {
            id: String,
            function: OpenAIFunctionCall,
        }

        #[derive(Deserialize)]
        struct OpenAIFunctionCall {
            name: String,
            arguments: String,
        }

        let tools: Vec<OpenAITool> = actions
            .iter()
            .map(|s| OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunction {
                    name: s.name.clone(),
                    description: s.description.clone(),
                    parameters: s.input_schema.clone(),
                },
            })
            .collect();

        // Build messages with system prompt first
        let mut messages = vec![OpenAIMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
        }];

        // Add conversation history (excluding the current message)
        for msg in history.iter().filter(|m| !(m.role == "user" && m.content == user_message)) {
            messages.push(OpenAIMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
            });
        }

        // Add current user message
        messages.push(OpenAIMessage {
            role: "user".to_string(),
            content: user_message.to_string(),
        });

        let request = OpenAIRequest {
            model: model.to_string(),
            messages,
            tools,
        };

        let url = base_url.unwrap_or("https://api.openai.com/v1");
        let response = self
            .client
            .post(format!("{}/chat/completions", url))
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(anyhow!("OpenAI API error: {}", error));
        }

        let response: OpenAIResponse = response.json().await?;
        let choice = response.choices.into_iter().next().ok_or_else(|| {
            anyhow!("No response from OpenAI")
        })?;

        let content = choice.message.content.unwrap_or_default();
        let tool_calls = choice
            .message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tc| ToolCall {
                id: tc.id,
                name: tc.function.name,
                arguments: serde_json::from_str(&tc.function.arguments)
                    .unwrap_or(serde_json::Value::Null),
            })
            .collect();

        Ok(LlmResponse {
            content,
            tool_calls,
        })
    }

    async fn chat_ollama_with_history(
        &self,
        base_url: &str,
        model: &str,
        system_prompt: &str,
        user_message: &str,
        history: &[crate::core::agent::ConversationMessage],
    ) -> Result<LlmResponse> {
        #[derive(Serialize)]
        struct OllamaRequest {
            model: String,
            messages: Vec<OllamaMessage>,
            stream: bool,
        }

        #[derive(Serialize, Deserialize)]
        struct OllamaMessage {
            role: String,
            content: String,
        }

        #[derive(Deserialize)]
        struct OllamaResponse {
            message: OllamaMessage,
        }

        // Build messages with system prompt first
        let mut messages = vec![OllamaMessage {
            role: "system".to_string(),
            content: system_prompt.to_string(),
        }];

        // Add conversation history
        for msg in history.iter().filter(|m| !(m.role == "user" && m.content == user_message)) {
            messages.push(OllamaMessage {
                role: msg.role.clone(),
                content: msg.content.clone(),
            });
        }

        // Add current user message
        messages.push(OllamaMessage {
            role: "user".to_string(),
            content: user_message.to_string(),
        });

        let request = OllamaRequest {
            model: model.to_string(),
            messages,
            stream: false,
        };

        let response = self
            .client
            .post(format!("{}/api/chat", base_url))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error = response.text().await?;
            return Err(anyhow!("Ollama API error: {}", error));
        }

        let response: OllamaResponse = response.json().await?;

        Ok(LlmResponse {
            content: response.message.content,
            tool_calls: vec![],
        })
    }
}
