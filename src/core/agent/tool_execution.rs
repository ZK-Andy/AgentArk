use super::*;

impl Agent {
    fn canonicalize_json_value(value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => {
                let mut keys: Vec<&String> = map.keys().collect();
                keys.sort_unstable();
                let mut ordered = serde_json::Map::new();
                for key in keys {
                    if let Some(inner) = map.get(key) {
                        ordered.insert(key.clone(), Self::canonicalize_json_value(inner));
                    }
                }
                serde_json::Value::Object(ordered)
            }
            serde_json::Value::Array(items) => serde_json::Value::Array(
                items
                    .iter()
                    .map(Self::canonicalize_json_value)
                    .collect::<Vec<_>>(),
            ),
            _ => value.clone(),
        }
    }

    fn tool_call_signature(call: &crate::core::llm::ToolCall) -> String {
        let canonical_args = Self::canonicalize_json_value(&call.arguments);
        let args = serde_json::to_string(&canonical_args).unwrap_or_else(|_| "{}".to_string());
        format!("{}:{}", call.name, args)
    }

    fn extract_output_route_components(url: &str) -> Option<(String, String)> {
        let path = if url.starts_with("http://") || url.starts_with("https://") {
            match reqwest::Url::parse(url) {
                Ok(parsed) => parsed.path().to_string(),
                Err(_) => return None,
            }
        } else {
            url.to_string()
        };
        let marker = "/api/outputs/";
        let idx = path.find(marker)?;
        let tail = &path[idx + marker.len()..];
        let mut parts = tail.splitn(2, '/');
        let exec_id = parts.next()?.trim().to_string();
        let filename = parts.next()?.trim().to_string();
        if exec_id.is_empty() || filename.is_empty() {
            return None;
        }
        let filename = match urlencoding::decode(&filename) {
            Ok(v) => v.to_string(),
            Err(_) => filename,
        };
        Some((exec_id, filename))
    }

    async fn load_video_bytes(&self, source_url: &str, max_bytes: usize) -> Result<Vec<u8>> {
        if source_url.starts_with("data:") {
            if let Some(comma_idx) = source_url.find(',') {
                let (meta, payload) = source_url.split_at(comma_idx);
                let payload = &payload[1..];
                if meta.contains(";base64") {
                    use base64::Engine;
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(payload.as_bytes())
                        .map_err(|e| anyhow::anyhow!("Failed to decode data URL video: {}", e))?;
                    if bytes.len() > max_bytes {
                        anyhow::bail!(
                            "Video too large for channel delivery: {} bytes (max {})",
                            bytes.len(),
                            max_bytes
                        );
                    }
                    return Ok(bytes);
                }
            }
            anyhow::bail!("Unsupported data URL video format");
        }

        if let Some((exec_id, filename)) = Self::extract_output_route_components(source_url) {
            if uuid::Uuid::parse_str(&exec_id).is_ok()
                && !filename.contains('/')
                && !filename.contains('\\')
                && !filename.contains("..")
            {
                let path = self.data_dir.join("outputs").join(exec_id).join(filename);
                let bytes = tokio::fs::read(&path).await?;
                if bytes.len() > max_bytes {
                    anyhow::bail!(
                        "Video too large for channel delivery: {} bytes (max {})",
                        bytes.len(),
                        max_bytes
                    );
                }
                return Ok(bytes);
            }
        }

        if source_url.starts_with("http://") || source_url.starts_with("https://") {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(90))
                .build()?;
            let resp = client.get(source_url).send().await?;
            if !resp.status().is_success() {
                anyhow::bail!("Failed to fetch video URL (status {})", resp.status());
            }
            if let Some(len) = resp.content_length() {
                if len > max_bytes as u64 {
                    anyhow::bail!(
                        "Video too large for channel delivery: {} bytes (max {})",
                        len,
                        max_bytes
                    );
                }
            }
            let bytes = resp.bytes().await?.to_vec();
            if bytes.len() > max_bytes {
                anyhow::bail!(
                    "Video too large for channel delivery: {} bytes (max {})",
                    bytes.len(),
                    max_bytes
                );
            }
            return Ok(bytes);
        }

        anyhow::bail!("Unsupported video URL format for delivery")
    }

    async fn extract_video_preview_from_bytes(&self, video_bytes: &[u8]) -> Result<Vec<u8>> {
        let temp_dir = std::env::temp_dir().join(format!("video-preview-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await?;
        let input_path = temp_dir.join("input.mp4");
        let output_path = temp_dir.join("preview.jpg");
        tokio::fs::write(&input_path, video_bytes).await?;

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(45),
            tokio::process::Command::new("ffmpeg")
                .args([
                    "-y",
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-ss",
                    "00:00:00.500",
                    "-i",
                    &input_path.to_string_lossy(),
                    "-frames:v",
                    "1",
                    "-vf",
                    "scale=960:-1",
                    &output_path.to_string_lossy(),
                ])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("ffmpeg preview extraction timed out"))??;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("ffmpeg preview extraction failed: {}", stderr);
        }
        let preview = tokio::fs::read(&output_path).await?;
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        Ok(preview)
    }

    async fn validate_and_capture_app_preview(
        &self,
        app_url_with_key: &str,
        app_id: &str,
        stream_tx: Option<&tokio::sync::mpsc::Sender<StreamEvent>>,
    ) -> Result<(Option<String>, bool, usize, String)> {
        const MAX_APP_VERIFY_ATTEMPTS: usize = 3;

        if !self.browser_sessions.is_available().await {
            return Ok((
                None,
                false,
                0,
                "Playwright sidecar unavailable (cannot auto-validate preview)".to_string(),
            ));
        }

        let integration = self.browser_sessions.integration().clone();
        let mut last_error = "Unknown validation error".to_string();

        for attempt in 1..=MAX_APP_VERIFY_ATTEMPTS {
            if let Some(tx) = stream_tx {
                let _ = tx.try_send(StreamEvent::ToolResult {
                    name: "app_deploy".to_string(),
                    content: format!(
                        "Validating deployed app (attempt {}/{})",
                        attempt, MAX_APP_VERIFY_ATTEMPTS
                    ),
                });
            }

            let sidecar_session = match integration.create_session().await {
                Ok(s) => s,
                Err(e) => {
                    last_error = format!("create_session failed: {}", e);
                    continue;
                }
            };

            let attempt_result: Result<String> = async {
                let _ = integration
                    .navigate(&sidecar_session, app_url_with_key)
                    .await?;
                tokio::time::sleep(std::time::Duration::from_millis(1200)).await;

                let content = integration.get_content(&sidecar_session).await?;
                let combined = format!("{}\n{}", content.title, content.body_text).to_lowercase();
                let lock_page_detected = combined.contains("access key required")
                    || (combined.contains("enter access key") && combined.contains("unlock"));
                if lock_page_detected {
                    anyhow::bail!("app opened in locked mode");
                }

                let screenshot = integration.screenshot(&sidecar_session).await?;
                if screenshot.is_empty() {
                    anyhow::bail!("empty screenshot returned");
                }
                self.persist_app_preview_screenshot(app_id, &screenshot)
                    .await
            }
            .await;

            let _ = integration.close_session(&sidecar_session).await;

            match attempt_result {
                Ok(screenshot_url) => {
                    return Ok((
                        Some(screenshot_url),
                        true,
                        attempt,
                        format!("Validated on attempt {}", attempt),
                    ));
                }
                Err(e) => {
                    last_error = e.to_string();
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            }
        }

        Ok((None, false, MAX_APP_VERIFY_ATTEMPTS, last_error))
    }

    async fn append_moltbook_tool_activity(
        &self,
        sub_action: &str,
        args: &serde_json::Value,
        result: Option<&serde_json::Value>,
        error: Option<&str>,
    ) {
        let mut events: Vec<serde_json::Value> = self
            .storage
            .get(MOLTBOOK_ACTIVITY_LOG_KEY)
            .await
            .ok()
            .flatten()
            .and_then(|raw| serde_json::from_slice::<Vec<serde_json::Value>>(&raw).ok())
            .unwrap_or_default();

        let urls = collect_moltbook_urls(sub_action, args, result);
        let action_kind = moltbook_action_kind(sub_action);

        let mut details = serde_json::json!({
            "source": "tool_call",
            "sub_action": sub_action,
            "action_kind": action_kind,
            "urls": urls
        });
        if let Some(post_id) = args.get("post_id").and_then(|v| v.as_str()) {
            details["post_id"] = serde_json::Value::String(post_id.to_string());
        }
        if let Some(submolt) = args.get("submolt").and_then(|v| v.as_str()) {
            details["submolt"] = serde_json::Value::String(submolt.to_string());
        }
        if let Some(query) = args.get("query").and_then(|v| v.as_str()) {
            details["query_preview"] = serde_json::Value::String(safe_truncate(query, 120));
        }
        if let Some(content) = args.get("content").and_then(|v| v.as_str()) {
            details["content_chars"] = serde_json::Value::from(content.chars().count() as u64);
        }
        if let Some(title) = args.get("title").and_then(|v| v.as_str()) {
            details["title_preview"] = serde_json::Value::String(safe_truncate(title, 120));
        }
        if let Some(err) = error {
            details["error"] = serde_json::Value::String(safe_truncate(err, 300));
        }
        if let Some(post_id) = result
            .and_then(|r| r.get("post"))
            .and_then(|p| p.get("id"))
            .and_then(|v| v.as_str())
        {
            details["result_post_id"] = serde_json::Value::String(post_id.to_string());
        }

        events.push(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "run_id": uuid::Uuid::new_v4().to_string(),
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "level": if error.is_some() { "error" } else { "info" },
            "action": format!("tool_{}", sub_action),
            "details": details
        }));
        if events.len() > MOLTBOOK_ACTIVITY_LOG_LIMIT {
            let drop = events.len() - MOLTBOOK_ACTIVITY_LOG_LIMIT;
            events.drain(0..drop);
        }
        if let Ok(bytes) = serde_json::to_vec(&events) {
            let _ = self.storage.set(MOLTBOOK_ACTIVITY_LOG_KEY, &bytes).await;
        }
    }

    async fn fire_action_hook(
        &self,
        trigger: crate::hooks::HookTrigger,
        channel: &str,
        action_name: &str,
        message_hint: Option<&str>,
        response: Option<&str>,
        event_id: &str,
    ) {
        self.hooks
            .fire(
                trigger.clone(),
                crate::hooks::HookContext {
                    event_id: Some(event_id.to_string()),
                    trigger: match trigger {
                        crate::hooks::HookTrigger::PreMessage => "pre_message".to_string(),
                        crate::hooks::HookTrigger::PostMessage => "post_message".to_string(),
                        crate::hooks::HookTrigger::PreAction => "pre_action".to_string(),
                        crate::hooks::HookTrigger::PostAction => "post_action".to_string(),
                        crate::hooks::HookTrigger::OnConsolidate => "on_consolidate".to_string(),
                        crate::hooks::HookTrigger::OnError => "on_error".to_string(),
                    },
                    channel: channel.to_string(),
                    message: message_hint.map(|m| safe_truncate(m, 500)),
                    response: response.map(|r| safe_truncate(r, 1500)),
                    action: Some(action_name.to_string()),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                },
            )
            .await;
    }

    pub(crate) async fn execute_action_with_hooks(
        &self,
        action_name: &str,
        arguments: &serde_json::Value,
        channel: &str,
        message_hint: Option<&str>,
    ) -> Result<String> {
        let event_id = uuid::Uuid::new_v4().to_string();
        self.fire_action_hook(
            crate::hooks::HookTrigger::PreAction,
            channel,
            action_name,
            message_hint,
            None,
            &event_id,
        )
        .await;

        match self.runtime.execute_action(action_name, arguments).await {
            Ok(result) => {
                self.fire_action_hook(
                    crate::hooks::HookTrigger::PostAction,
                    channel,
                    action_name,
                    message_hint,
                    Some(&result),
                    &event_id,
                )
                .await;
                Ok(result)
            }
            Err(e) => {
                let err_text = e.to_string();
                self.fire_action_hook(
                    crate::hooks::HookTrigger::OnError,
                    channel,
                    action_name,
                    message_hint,
                    Some(&err_text),
                    &event_id,
                )
                .await;
                Err(e)
            }
        }
    }

    fn sanitize_stream_preview(&self, text: &str) -> String {
        let filtered = self.security.filter_output(text);
        safe_truncate(&filtered.text, 300)
    }

    async fn load_public_base_url(&self) -> Option<String> {
        self.storage
            .get("public_base_url")
            .await
            .ok()
            .flatten()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                std::env::var("AGENTARK_PUBLIC_BASE_URL")
                    .ok()
                    .map(|s| s.trim().trim_end_matches('/').to_string())
                    .filter(|s| !s.is_empty())
            })
    }

    fn absolutize_public_url(public_base_url: Option<&str>, url: &str) -> String {
        if url.starts_with("http://")
            || url.starts_with("https://")
            || url.starts_with("data:")
            || url.starts_with("blob:")
        {
            return url.to_string();
        }
        if let Some(base) = public_base_url {
            if url.starts_with('/') {
                return format!("{}{}", base, url);
            }
            return format!("{}/{}", base, url);
        }
        url.to_string()
    }

    fn default_tool_integration_aliases() -> HashMap<String, String> {
        let mut aliases = HashMap::new();
        aliases.insert("github".to_string(), "github".to_string());
        aliases.insert("notion".to_string(), "notion".to_string());
        aliases.insert("twitter".to_string(), "twitter".to_string());
        aliases.insert("onepassword".to_string(), "onepassword".to_string());
        aliases.insert("places".to_string(), "google_places".to_string());
        aliases.insert("twilio".to_string(), "twilio".to_string());
        aliases.insert("ordering".to_string(), "ordering".to_string());
        aliases.insert("garmin".to_string(), "garmin".to_string());
        aliases.insert("whoop".to_string(), "whoop".to_string());
        aliases.insert("ga4".to_string(), "ga4".to_string());
        aliases.insert("gsc".to_string(), "gsc".to_string());
        aliases.insert("social_analytics".to_string(), "social_analytics".to_string());
        aliases.insert("moltbook".to_string(), "moltbook".to_string());
        aliases
    }

    fn merge_tool_integration_aliases(
        aliases: &mut HashMap<String, String>,
        value: &serde_json::Value,
    ) {
        let Some(obj) = value.as_object() else {
            return;
        };
        for (tool_name, integration_id_value) in obj {
            let Some(integration_id) = integration_id_value.as_str() else {
                continue;
            };
            let tool_name = tool_name.trim();
            let integration_id = integration_id.trim();
            if tool_name.is_empty() || integration_id.is_empty() {
                continue;
            }
            aliases.insert(tool_name.to_string(), integration_id.to_string());
        }
    }

    async fn load_tool_integration_aliases(&self) -> HashMap<String, String> {
        let mut aliases = Self::default_tool_integration_aliases();

        if let Ok(raw_env) = std::env::var("AGENTARK_TOOL_INTEGRATION_ALIASES") {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw_env) {
                Self::merge_tool_integration_aliases(&mut aliases, &value);
            } else {
                tracing::warn!("Invalid AGENTARK_TOOL_INTEGRATION_ALIASES JSON ignored");
            }
        }

        if let Ok(Some(raw)) = self.storage.get(TOOL_INTEGRATION_ALIASES_KEY).await {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&raw) {
                Self::merge_tool_integration_aliases(&mut aliases, &value);
            } else {
                tracing::warn!("Invalid '{}' JSON ignored", TOOL_INTEGRATION_ALIASES_KEY);
            }
        }

        aliases
    }

    async fn load_persisted_tool_integration_aliases(&self) -> HashMap<String, String> {
        let Ok(Some(raw)) = self.storage.get(TOOL_INTEGRATION_ALIASES_KEY).await else {
            return HashMap::new();
        };
        serde_json::from_slice::<HashMap<String, String>>(&raw).unwrap_or_default()
    }

    pub(crate) async fn register_tool_integration_alias(
        &self,
        tool_name: &str,
        integration_id: &str,
    ) -> Result<()> {
        let tool_name = tool_name.trim();
        let integration_id = integration_id.trim();
        if tool_name.is_empty() || integration_id.is_empty() {
            return Err(anyhow::anyhow!(
                "tool_name and integration_id must be non-empty"
            ));
        }
        let mut persisted = self.load_persisted_tool_integration_aliases().await;
        persisted.insert(tool_name.to_string(), integration_id.to_string());
        let raw = serde_json::to_vec(&persisted)?;
        self.storage.set(TOOL_INTEGRATION_ALIASES_KEY, &raw).await?;
        Ok(())
    }

    pub(crate) fn resolve_tool_integration_id(
        &self,
        tool_name: &str,
        aliases: &HashMap<String, String>,
    ) -> Option<String> {
        if let Some(mapped) = aliases.get(tool_name) {
            return Some(mapped.clone());
        }
        if self.integrations.get(tool_name).is_some() {
            return Some(tool_name.to_string());
        }
        None
    }

    pub(crate) async fn execute_integration_tool_call(
        &self,
        call: &crate::core::llm::ToolCall,
        stream_tx: Option<&tokio::sync::mpsc::Sender<StreamEvent>>,
        request_channel: &str,
        integration_id: &str,
    ) -> String {
        let sub_action = call
            .arguments
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("default");
        let resolved_args = self
            .runtime
            .resolve_secret_placeholders(&call.name, &call.arguments)
            .unwrap_or_else(|_| call.arguments.clone());
        let hook_event_id = uuid::Uuid::new_v4().to_string();
        let hook_hint = action_message_hint(&resolved_args);
        self.fire_action_hook(
            crate::hooks::HookTrigger::PreAction,
            request_channel,
            &call.name,
            hook_hint.as_deref(),
            None,
            &hook_event_id,
        )
        .await;

        match self
            .integrations
            .execute(integration_id, sub_action, &resolved_args)
            .await
        {
            Ok(result) => {
                if integration_id == "moltbook" {
                    self.append_moltbook_tool_activity(sub_action, &resolved_args, Some(&result), None)
                        .await;
                }
                let formatted =
                    serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
                self.fire_action_hook(
                    crate::hooks::HookTrigger::PostAction,
                    request_channel,
                    &call.name,
                    hook_hint.as_deref(),
                    Some(&formatted),
                    &hook_event_id,
                )
                .await;
                if let Some(tx) = stream_tx {
                    let _ = tx.try_send(StreamEvent::ToolResult {
                        name: call.name.clone(),
                        content: self.sanitize_stream_preview(&formatted),
                    });
                }
                formatted
            }
            Err(e) => {
                if integration_id == "moltbook" {
                    self.append_moltbook_tool_activity(
                        sub_action,
                        &resolved_args,
                        None,
                        Some(&e.to_string()),
                    )
                    .await;
                }
                tracing::error!("{} integration error: {}", call.name, e);
                self.fire_action_hook(
                    crate::hooks::HookTrigger::OnError,
                    request_channel,
                    &call.name,
                    hook_hint.as_deref(),
                    Some(&e.to_string()),
                    &hook_event_id,
                )
                .await;
                let formatted = format!("Error from {}: {}", call.name, e);
                if let Some(tx) = stream_tx {
                    let _ = tx.try_send(StreamEvent::ToolResult {
                        name: call.name.clone(),
                        content: formatted.clone(),
                    });
                }
                formatted
            }
        }
    }

    fn integration_capability_labels(
        caps: Vec<crate::integrations::Capability>,
    ) -> Vec<String> {
        caps.into_iter()
            .map(|cap| match cap {
                crate::integrations::Capability::Read => "read".to_string(),
                crate::integrations::Capability::Write => "write".to_string(),
                crate::integrations::Capability::Subscribe => "subscribe".to_string(),
                crate::integrations::Capability::Search => "search".to_string(),
                crate::integrations::Capability::Delete => "delete".to_string(),
                crate::integrations::Capability::Notify => "notify".to_string(),
            })
            .collect()
    }

    fn build_integration_action_def(
        &self,
        tool_name: &str,
        integration_id: &str,
        integration: &dyn crate::integrations::Integration,
    ) -> crate::actions::ActionDef {
        crate::actions::ActionDef {
            name: tool_name.to_string(),
            description: format!(
                "Integration tool '{}' routed to '{}'. {} Pass an 'action' field and any connector-specific parameters.",
                tool_name,
                integration_id,
                integration.description()
            ),
            version: "1.0.0".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "description": "Connector operation to execute"
                    }
                },
                "additionalProperties": true
            }),
            capabilities: Self::integration_capability_labels(integration.capabilities()),
            sandbox_mode: None,
            source: crate::actions::ActionSource::System,
            file_path: None,
        }
    }

    pub(crate) async fn append_dynamic_integration_actions(
        &self,
        actions: &mut Vec<crate::actions::ActionDef>,
    ) {
        let mut existing: HashSet<String> = actions.iter().map(|a| a.name.clone()).collect();
        let integration_aliases = self.load_tool_integration_aliases().await;

        for integration_id in self.integrations.ids() {
            let Some(integration) = self.integrations.get(&integration_id) else {
                continue;
            };
            if existing.insert(integration_id.clone()) {
                actions.push(self.build_integration_action_def(
                    &integration_id,
                    &integration_id,
                    integration,
                ));
            }
        }

        for (tool_name, integration_id) in integration_aliases {
            if !existing.insert(tool_name.clone()) {
                continue;
            }
            let Some(integration) = self.integrations.get(&integration_id) else {
                continue;
            };
            actions.push(self.build_integration_action_def(
                &tool_name,
                &integration_id,
                integration,
            ));
        }
    }

    pub(crate) async fn execute_single_tool_call_legacy(
        &self,
        call: &crate::core::llm::ToolCall,
        trace_ref: &Arc<RwLock<ExecutionTrace>>,
        stream_tx: Option<tokio::sync::mpsc::Sender<StreamEvent>>,
        request_channel: &str,
    ) -> Result<String> {
        let synthetic = crate::core::llm::LlmResponse {
            content: String::new(),
            tool_calls: vec![call.clone()],
            reasoning: None,
            usage: None,
            provider: "internal".to_string(),
            model: "tool_dispatch".to_string(),
        };
        self.execute_tool_calls_legacy(&synthetic, trace_ref, stream_tx, request_channel)
            .await
    }

    pub(crate) async fn handle_generate_image_tool_call(
        &self,
        call: &crate::core::llm::ToolCall,
        stream_tx: Option<&tokio::sync::mpsc::Sender<StreamEvent>>,
        request_channel: &str,
    ) -> Result<String> {
        self.execute_single_tool_call_legacy(
            call,
            &Arc::new(RwLock::new(ExecutionTrace::default())),
            stream_tx.cloned(),
            request_channel,
        )
        .await
    }

    pub(crate) async fn handle_generate_video_tool_call(
        &self,
        call: &crate::core::llm::ToolCall,
        stream_tx: Option<&tokio::sync::mpsc::Sender<StreamEvent>>,
        request_channel: &str,
        _public_base_url: Option<&str>,
    ) -> Result<String> {
        self.execute_single_tool_call_legacy(
            call,
            &Arc::new(RwLock::new(ExecutionTrace::default())),
            stream_tx.cloned(),
            request_channel,
        )
        .await
    }

    pub(crate) async fn handle_browser_auto_tool_call(
        &self,
        call: &crate::core::llm::ToolCall,
        stream_tx: Option<&tokio::sync::mpsc::Sender<StreamEvent>>,
    ) -> Result<String> {
        self.execute_single_tool_call_legacy(
            call,
            &Arc::new(RwLock::new(ExecutionTrace::default())),
            stream_tx.cloned(),
            "web",
        )
        .await
    }

    pub(crate) async fn handle_app_deploy_tool_call(
        &self,
        call: &crate::core::llm::ToolCall,
        stream_tx: Option<&tokio::sync::mpsc::Sender<StreamEvent>>,
        request_channel: &str,
        _public_base_url: Option<&str>,
    ) -> Result<String> {
        self.execute_single_tool_call_legacy(
            call,
            &Arc::new(RwLock::new(ExecutionTrace::default())),
            stream_tx.cloned(),
            request_channel,
        )
        .await
    }

    pub(crate) async fn handle_runtime_tool_call(
        &self,
        call: &crate::core::llm::ToolCall,
        trace_ref: &Arc<RwLock<ExecutionTrace>>,
        stream_tx: Option<&tokio::sync::mpsc::Sender<StreamEvent>>,
        request_channel: &str,
        _public_base_url: Option<&str>,
    ) -> Result<String> {
        self.execute_single_tool_call_legacy(call, trace_ref, stream_tx.cloned(), request_channel)
            .await
    }

    /// Take a screenshot of a URL using the Playwright sidecar.
    pub(crate) async fn handle_screenshot_tool_call(
        &self,
        call: &crate::core::llm::ToolCall,
        stream_tx: Option<&tokio::sync::mpsc::Sender<StreamEvent>>,
        request_channel: &str,
    ) -> Result<String> {
        let url = call
            .arguments
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if url.is_empty() {
            return Ok(
                serde_json::json!({"status": "error", "message": "Missing required 'url' parameter"})
                    .to_string(),
            );
        }

        let wait_ms = call
            .arguments
            .get("wait_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(1500);

        if let Some(tx) = stream_tx {
            let _ = tx.try_send(StreamEvent::ToolStart {
                name: "page_screenshot".to_string(),
            });
        }

        if !self.browser_sessions.is_available().await {
            return Ok(
                serde_json::json!({"status": "error", "message": "Playwright sidecar unavailable"})
                    .to_string(),
            );
        }

        let integration = self.browser_sessions.integration().clone();
        let session = integration.create_session().await.map_err(|e| {
            anyhow::anyhow!("Failed to create browser session: {}", e)
        })?;

        let result: Result<String> = async {
            let _ = integration.navigate(&session, &url).await?;
            tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;

            let screenshot = integration.screenshot(&session).await?;
            if screenshot.is_empty() {
                anyhow::bail!("Empty screenshot returned");
            }

            let screenshot_url = self
                .persist_output_binary("screenshot", "png", &screenshot)
                .await?;

            // Send to channel if not web
            if request_channel != "web" {
                let _ = crate::channels::send_screenshot(
                    self,
                    request_channel,
                    &screenshot,
                    &format!("Screenshot of {}", url),
                    Some(&screenshot_url),
                )
                .await;
            }

            Ok(serde_json::json!({
                "status": "ok",
                "url": screenshot_url,
                "size_bytes": screenshot.len()
            })
            .to_string())
        }
        .await;

        let _ = integration.close_session(&session).await;

        let output = match result {
            Ok(json) => json,
            Err(e) => {
                serde_json::json!({"status": "error", "message": e.to_string()}).to_string()
            }
        };

        if let Some(tx) = stream_tx {
            let _ = tx.try_send(StreamEvent::ToolResult {
                name: "page_screenshot".to_string(),
                content: output.clone(),
            });
        }

        Ok(output)
    }

    /// Compose a structured report as HTML or Markdown.
    pub(crate) async fn handle_compose_report_tool_call(
        &self,
        call: &crate::core::llm::ToolCall,
        stream_tx: Option<&tokio::sync::mpsc::Sender<StreamEvent>>,
    ) -> Result<String> {
        let title = call
            .arguments
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Report")
            .to_string();
        let sections = call
            .arguments
            .get("sections")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let format = call
            .arguments
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("html")
            .to_string();

        if let Some(tx) = stream_tx {
            let _ = tx.try_send(StreamEvent::ToolStart {
                name: "compose_report".to_string(),
            });
        }

        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();

        let output = if format == "markdown" {
            let mut md = format!("# {}\n\n*Generated: {}*\n\n", title, timestamp);
            for section in &sections {
                let header = section
                    .get("header")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Section");
                let content = section
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                md.push_str(&format!("## {}\n\n{}\n\n", header, content));
            }

            let report_url = self
                .persist_output_binary("report", "md", md.as_bytes())
                .await?;

            serde_json::json!({
                "status": "ok",
                "path": report_url,
                "format": "markdown"
            })
            .to_string()
        } else {
            // HTML report with dark-themed inline CSS
            let mut body_html = String::new();
            for section in &sections {
                let header = section
                    .get("header")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Section");
                let content = section
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                // Convert newlines in content to <br> for display
                let content_html = content
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
                    .replace('\n', "<br>");
                body_html.push_str(&format!(
                    "<section><h2>{}</h2><div class=\"content\">{}</div></section>\n",
                    header
                        .replace('&', "&amp;")
                        .replace('<', "&lt;")
                        .replace('>', "&gt;"),
                    content_html
                ));
            }

            let html = format!(
                r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
         background: #0a0e1a; color: #e0e6f0; padding: 2rem; line-height: 1.6; }}
  .report {{ max-width: 800px; margin: 0 auto; }}
  h1 {{ font-size: 1.8rem; margin-bottom: 0.25rem; color: #fff; }}
  .timestamp {{ font-size: 0.85rem; color: #6b7a99; margin-bottom: 2rem; }}
  section {{ background: rgba(255,255,255,0.04); border: 1px solid rgba(255,255,255,0.08);
            border-radius: 12px; padding: 1.25rem 1.5rem; margin-bottom: 1rem; }}
  h2 {{ font-size: 1.15rem; color: #2fd4ff; margin-bottom: 0.75rem; }}
  .content {{ color: #c0c8d8; }}
</style>
</head>
<body>
<div class="report">
  <h1>{title}</h1>
  <div class="timestamp">{timestamp}</div>
  {body_html}
</div>
</body>
</html>"#,
                title = title
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;"),
                timestamp = timestamp,
                body_html = body_html,
            );

            let report_url = self
                .persist_output_binary("report", "html", html.as_bytes())
                .await?;

            serde_json::json!({
                "status": "ok",
                "path": report_url,
                "format": "html"
            })
            .to_string()
        };

        if let Some(tx) = stream_tx {
            let _ = tx.try_send(StreamEvent::ToolResult {
                name: "compose_report".to_string(),
                content: output.clone(),
            });
        }

        Ok(output)
    }

    /// Execute tool calls from LLM response using modular handler dispatch.
    pub(crate) async fn execute_tool_calls(
        &self,
        response: &crate::core::llm::LlmResponse,
        trace_ref: &Arc<RwLock<ExecutionTrace>>,
        stream_tx: Option<tokio::sync::mpsc::Sender<StreamEvent>>,
        request_channel: &str,
    ) -> Result<String> {
        if response.tool_calls.is_empty() {
            return Ok(response.content.clone());
        }

        let public_base_url = self.load_public_base_url().await;
        let integration_aliases = self.load_tool_integration_aliases().await;
        let handlers = default_tool_handlers();

        let mut seen_signatures: HashSet<String> = HashSet::new();
        let mut unique_calls: Vec<&crate::core::llm::ToolCall> = Vec::new();
        for call in &response.tool_calls {
            let sig = Self::tool_call_signature(call);
            if seen_signatures.insert(sig) {
                unique_calls.push(call);
            }
        }

        let mut results = Vec::new();
        for call in unique_calls {
            let ctx = ToolHandlerContext {
                trace_ref,
                stream_tx: stream_tx.as_ref(),
                request_channel,
                public_base_url: public_base_url.as_deref(),
                integration_aliases: &integration_aliases,
            };

            let mut handled = false;
            for handler in &handlers {
                if !handler.can_handle(self, call, &ctx) {
                    continue;
                }
                tracing::debug!("Tool '{}' handled by '{}'", call.name, handler.id());
                if let Some(output) = handler.handle(self, call, &ctx).await? {
                    results.push(output);
                    handled = true;
                    break;
                }
            }

            if !handled {
                let msg = format!("No handler registered for tool '{}'", call.name);
                if let Some(ref tx) = stream_tx {
                    let _ = tx.try_send(StreamEvent::ToolResult {
                        name: call.name.clone(),
                        content: msg.clone(),
                    });
                }
                results.push(msg);
            }
        }

        if response.content.is_empty() {
            Ok(results.join("\n"))
        } else {
            Ok(format!("{}\n\n{}", response.content, results.join("\n")))
        }
    }

    /// Legacy monolithic tool execution path. New dispatchers route through
    /// modular handlers and can gradually replace this implementation.
    pub(crate) async fn execute_tool_calls_legacy(
        &self,
        response: &crate::core::llm::LlmResponse,
        trace_ref: &Arc<RwLock<ExecutionTrace>>,
        stream_tx: Option<tokio::sync::mpsc::Sender<StreamEvent>>,
        request_channel: &str,
    ) -> Result<String> {
        if response.tool_calls.is_empty() {
            return Ok(response.content.clone());
        }

        let mut results = Vec::new();
        let sanitize_stream = |s: &str| -> String { self.sanitize_stream_preview(s) };
        let public_base_url = self.load_public_base_url().await;
        let integration_aliases = self.load_tool_integration_aliases().await;
        let absolutize_url = |url: &str| -> String {
            Self::absolutize_public_url(public_base_url.as_deref(), url)
        };

        // Deduplicate repeated tool calls (same name + identical args) so app_deploy
        // and other side-effecting actions do not run twice from merged paths.
        let mut seen_signatures: HashSet<String> = HashSet::new();
        let mut unique_calls: Vec<&crate::core::llm::ToolCall> = Vec::new();
        for call in &response.tool_calls {
            let sig = Self::tool_call_signature(call);
            if seen_signatures.insert(sig) {
                unique_calls.push(call);
            }
        }

        for call in unique_calls {
            if let Some(ref tx) = stream_tx {
                let _ = tx.try_send(StreamEvent::ToolStart {
                    name: call.name.clone(),
                });
            }

            // Check safety policy
            if !self.safety.is_allowed(&call.name, &call.arguments).await? {
                let blocked = format!("Tool '{}' blocked by safety policy", call.name);
                if let Some(ref tx) = stream_tx {
                    let _ = tx.try_send(StreamEvent::ToolResult {
                        name: call.name.clone(),
                        content: blocked.clone(),
                    });
                }
                results.push(blocked);
                continue;
            }

            // Handle generate_image via integrations (not runtime)
            if call.name == "generate_image" {
                // Inject configured model if not specified in the call
                let mut args = call.arguments.clone();
                if args.get("model").and_then(|v| v.as_str()).is_none() {
                    if let Some(ref model) = self.config.media_gen.image_model {
                        args["model"] = serde_json::Value::String(model.clone());
                    }
                }
                match self
                    .integrations
                    .execute("media_gen", "generate_image", &args)
                    .await
                {
                    Ok(result) => {
                        if let Some(url) = result.get("url").and_then(|v| v.as_str()) {
                            let provider = result
                                .get("provider")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let time_ms = result
                                .get("generation_time_ms")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            let formatted = format!(
                                "[IMAGE_RESULT]{}\n[/IMAGE_RESULT]\n*Generated by {} in {}ms*",
                                url, provider, time_ms
                            );
                            if let Some(ref tx) = stream_tx {
                                let _ = tx.try_send(StreamEvent::ToolResult {
                                    name: call.name.clone(),
                                    content: format!(
                                        "Generated image via {} ({}ms)",
                                        provider, time_ms
                                    ),
                                });
                            }
                            results.push(formatted);
                        } else {
                            let formatted = format!("Image generated: {}", result);
                            if let Some(ref tx) = stream_tx {
                                let _ = tx.try_send(StreamEvent::ToolResult {
                                    name: call.name.clone(),
                                    content: sanitize_stream(&formatted),
                                });
                            }
                            results.push(formatted);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Image generation error: {}", e);
                        let formatted = format!("Error generating image: {}", e);
                        if let Some(ref tx) = stream_tx {
                            let _ = tx.try_send(StreamEvent::ToolResult {
                                name: call.name.clone(),
                                content: formatted.clone(),
                            });
                        }
                        results.push(formatted);
                    }
                }
                continue;
            }

            // Handle provider-based video generation via integrations (not runtime)
            if call.name == "generate_video" {
                match self
                    .integrations
                    .execute("media_gen", "generate_video", &call.arguments)
                    .await
                {
                    Ok(result) => {
                        if let Some(url) = result.get("url").and_then(|v| v.as_str()) {
                            let provider = result
                                .get("provider")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let model = result
                                .get("model")
                                .and_then(|v| v.as_str())
                                .unwrap_or("default");
                            let mut source_url = url.to_string();
                            let mut video_bytes: Option<Vec<u8>> = None;

                            // Convert data URLs into persisted output files so links remain usable.
                            if source_url.starts_with("data:") {
                                match self.load_video_bytes(&source_url, 80 * 1024 * 1024).await {
                                    Ok(bytes) => {
                                        video_bytes = Some(bytes.clone());
                                        match self
                                            .persist_output_binary("provider_video", "mp4", &bytes)
                                            .await
                                        {
                                            Ok(local_url) => source_url = local_url,
                                            Err(e) => tracing::warn!(
                                                "Failed to persist provider data URL video: {}",
                                                e
                                            ),
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "Failed to decode provider data URL video: {}",
                                            e
                                        );
                                    }
                                }
                            }

                            let rendered_url = absolutize_url(&source_url);
                            let mut preview_url: Option<String> = None;

                            // Build preview screenshot for all provider videos.
                            if video_bytes.is_none() {
                                if let Ok(bytes) =
                                    self.load_video_bytes(&source_url, 45 * 1024 * 1024).await
                                {
                                    video_bytes = Some(bytes);
                                }
                            }
                            if let Some(bytes) = video_bytes.as_ref() {
                                match self.extract_video_preview_from_bytes(bytes).await {
                                    Ok(preview_bytes) => {
                                        if let Ok(rel) = self
                                            .persist_output_binary(
                                                "provider_video_preview",
                                                "jpg",
                                                &preview_bytes,
                                            )
                                            .await
                                        {
                                            let abs = absolutize_url(&rel);
                                            preview_url = Some(abs.clone());
                                            if matches!(request_channel, "telegram" | "whatsapp") {
                                                let _ = crate::channels::send_screenshot(
                                                    self,
                                                    request_channel,
                                                    &preview_bytes,
                                                    "Video preview",
                                                    Some(&abs),
                                                )
                                                .await;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "Failed to extract provider video preview: {}",
                                            e
                                        );
                                    }
                                }
                            }

                            // Direct attachment where reliable: Telegram and WhatsApp(Baileys).
                            let mut delivered_to_channel = false;
                            let whatsapp_baileys = self
                                .config
                                .whatsapp
                                .as_ref()
                                .map(|cfg| {
                                    matches!(
                                        cfg.mode,
                                        crate::channels::whatsapp::WhatsAppMode::Baileys
                                    )
                                })
                                .unwrap_or(false);
                            let should_direct_send = request_channel == "telegram"
                                || (request_channel == "whatsapp" && whatsapp_baileys);

                            if should_direct_send {
                                if video_bytes.is_none() {
                                    if let Ok(bytes) =
                                        self.load_video_bytes(&source_url, 80 * 1024 * 1024).await
                                    {
                                        video_bytes = Some(bytes);
                                    }
                                }
                                if let Some(bytes) = video_bytes.as_ref() {
                                    let caption =
                                        format!("Video generated by {} ({})", provider, model);
                                    if crate::channels::send_video_to_channel(
                                        self,
                                        request_channel,
                                        bytes,
                                        &caption,
                                        Some(&rendered_url),
                                    )
                                    .await
                                    .is_ok()
                                    {
                                        delivered_to_channel = true;
                                    }
                                }
                            }

                            let preview_text = preview_url
                                .as_ref()
                                .map(|u| format!("\nPreview: {}", u))
                                .unwrap_or_default();
                            let formatted = if matches!(request_channel, "telegram" | "whatsapp") {
                                if delivered_to_channel {
                                    format!(
                                        "Video sent to this chat.\nDownload: {}{}",
                                        rendered_url, preview_text
                                    )
                                } else {
                                    format!(
                                        "Video generated via {} ({}): {}\n{}",
                                        provider,
                                        model,
                                        rendered_url,
                                        if let Some(p) = preview_url.as_ref() {
                                            format!("Preview: {}", p)
                                        } else {
                                            "Preview unavailable".to_string()
                                        }
                                    )
                                }
                            } else if let Some(preview) = preview_url.as_ref() {
                                format!(
                                    "[VIDEO_RESULT]{}\n[/VIDEO_RESULT]\n[IMAGE_RESULT]{}\n[/IMAGE_RESULT]\n*Generated by {} ({})*",
                                    rendered_url, preview, provider, model
                                )
                            } else {
                                format!(
                                    "[VIDEO_RESULT]{}\n[/VIDEO_RESULT]\n*Generated by {} ({})*",
                                    rendered_url, provider, model
                                )
                            };
                            if let Some(ref tx) = stream_tx {
                                let _ = tx.try_send(StreamEvent::ToolResult {
                                    name: call.name.clone(),
                                    content: format!("Generated video via {}", provider),
                                });
                            }
                            results.push(formatted);
                        } else {
                            let formatted = format!("Video generated: {}", result);
                            if let Some(ref tx) = stream_tx {
                                let _ = tx.try_send(StreamEvent::ToolResult {
                                    name: call.name.clone(),
                                    content: sanitize_stream(&formatted),
                                });
                            }
                            results.push(formatted);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Provider video generation error: {}", e);
                        let formatted = format!("Error generating video: {}", e);
                        if let Some(ref tx) = stream_tx {
                            let _ = tx.try_send(StreamEvent::ToolResult {
                                name: call.name.clone(),
                                content: formatted.clone(),
                            });
                        }
                        results.push(formatted);
                    }
                }
                continue;
            }

            // Handle browser automation - starts a background session
            if call.name == "browser_auto" {
                let sub_action = call
                    .arguments
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("start_session");

                if sub_action == "start_session" {
                    let task_desc = call
                        .arguments
                        .get("task")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Browse the web");
                    let channel = call
                        .arguments
                        .get("channel")
                        .and_then(|v| v.as_str())
                        .unwrap_or("web");

                    if !self.browser_sessions.is_available().await {
                        tracing::warn!(
                            "Browser automation unavailable: Playwright sidecar not reachable"
                        );
                        let formatted = r#"{"error": "browser_unavailable", "detail": "Playwright sidecar is not running"}"#.to_string();
                        if let Some(ref tx) = stream_tx {
                            let _ = tx.try_send(StreamEvent::ToolResult {
                                name: call.name.clone(),
                                content: formatted.clone(),
                            });
                        }
                        results.push(formatted);
                        continue;
                    }

                    if self.browser_sessions.active_count() >= 2 {
                        tracing::warn!("Browser session limit reached: 2 active sessions");
                        let formatted = r#"{"error": "session_limit", "detail": "Maximum 2 concurrent browser sessions"}"#.to_string();
                        if let Some(ref tx) = stream_tx {
                            let _ = tx.try_send(StreamEvent::ToolResult {
                                name: call.name.clone(),
                                content: formatted.clone(),
                            });
                        }
                        results.push(formatted);
                        continue;
                    }

                    // Create a notification callback that sends messages to the user's channel
                    let chat_id = call
                        .arguments
                        .get("chat_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let notify_channel = channel.to_string();
                    let agent_config = self.config.clone();
                    let storage_clone = self.storage.clone();
                    let notify_fn: std::sync::Arc<dyn Fn(String, Option<Vec<u8>>) + Send + Sync> =
                        std::sync::Arc::new(move |msg: String, screenshot: Option<Vec<u8>>| {
                            let config = agent_config.clone();
                            let channel = notify_channel.clone();
                            let chat_id = chat_id.clone();
                            let storage = storage_clone.clone();
                            let _screenshot = screenshot; // screenshots sent via channel-specific methods
                            tokio::spawn(async move {
                                // Store as notification in DB so it appears in web UI
                                let notif = crate::storage::entities::notification::Model {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    title: "Browser Automation".to_string(),
                                    body: msg.clone(),
                                    level: "info".to_string(),
                                    source: "browser".to_string(),
                                    read: false,
                                    created_at: chrono::Utc::now().to_rfc3339(),
                                };
                                let _ = storage.insert_notification(&notif).await;

                                // Send to Telegram if configured
                                #[cfg(feature = "telegram")]
                                if channel == "telegram" {
                                    if let Some(tg) = &config.telegram {
                                        if !tg.bot_token.is_empty() {
                                            let target = if !chat_id.is_empty() {
                                                chat_id.parse::<i64>().unwrap_or(0)
                                            } else if let Some(first) = tg.allowed_users.first() {
                                                *first
                                            } else {
                                                0
                                            };
                                            if target != 0 {
                                                use teloxide::requests::Requester;
                                                let bot = teloxide::Bot::new(&tg.bot_token);
                                                let _ = bot
                                                    .send_message(
                                                        teloxide::types::ChatId(target),
                                                        &msg,
                                                    )
                                                    .await;
                                            }
                                        }
                                    }
                                }
                                let _ = channel; // suppress unused warning on non-telegram builds
                            });
                        });

                    let llm_clone = self.llm.clone();
                    match self
                        .browser_sessions
                        .start_session(task_desc, channel, "", llm_clone, notify_fn)
                        .await
                    {
                        Ok(session_id) => {
                            tracing::info!(
                                "Browser session started: session={}, task_len={}",
                                &session_id[..8],
                                task_desc.len()
                            );
                            // Return structured data - let the LLM craft the user message
                            if let Some(ref tx) = stream_tx {
                                let _ = tx.try_send(StreamEvent::ToolResult {
                                    name: call.name.clone(),
                                    content: format!(
                                        "Browser session started: {}",
                                        &session_id[..8]
                                    ),
                                });
                            }
                            results.push(format!(
                                r#"{{"status": "session_started", "session_id": "{}", "task": "{}"}}"#,
                                session_id, task_desc.replace('"', "'")
                            ));
                        }
                        Err(e) => {
                            tracing::error!("Browser session start failed: error={}", e);
                            if let Some(ref tx) = stream_tx {
                                let _ = tx.try_send(StreamEvent::ToolResult {
                                    name: call.name.clone(),
                                    content: format!("Browser session start failed: {}", e),
                                });
                            }
                            results.push(format!(
                                r#"{{"error": "session_start_failed", "detail": "{}"}}"#,
                                e
                            ));
                        }
                    }
                } else {
                    // Direct browser actions (for manual control)
                    let integration = self.browser_sessions.integration();
                    let resolved_args = self
                        .runtime
                        .resolve_secret_placeholders(&call.name, &call.arguments)
                        .unwrap_or_else(|_| call.arguments.clone());
                    match self
                        .integrations
                        .execute("browser", sub_action, &resolved_args)
                        .await
                    {
                        Ok(result) => {
                            let formatted = serde_json::to_string_pretty(&result)
                                .unwrap_or_else(|_| result.to_string());
                            if let Some(ref tx) = stream_tx {
                                let _ = tx.try_send(StreamEvent::ToolResult {
                                    name: call.name.clone(),
                                    content: sanitize_stream(&formatted),
                                });
                            }
                            results.push(formatted);
                        }
                        Err(e) => {
                            // Try via direct integration
                            let _ = integration; // used in future expansion
                            let formatted = format!("Browser action error: {}", e);
                            if let Some(ref tx) = stream_tx {
                                let _ = tx.try_send(StreamEvent::ToolResult {
                                    name: call.name.clone(),
                                    content: formatted.clone(),
                                });
                            }
                            results.push(formatted);
                        }
                    }
                }
                continue;
            }

            // Handle integration-backed tools via dynamic routing aliases + direct integration IDs.
            if let Some(integration_id) =
                self.resolve_tool_integration_id(&call.name, &integration_aliases)
            {
                let formatted = self
                    .execute_integration_tool_call(
                        call,
                        stream_tx.as_ref(),
                        request_channel,
                        &integration_id,
                    )
                    .await;
                results.push(formatted);
                continue;
            }

            // Handle app deployment - needs AppRegistry from agent
            if call.name == "app_deploy" {
                // Resolve secret placeholders for deployment-time env injection, without mutating
                // the original tool arguments (so traces stay safe).
                let resolved_args = self
                    .runtime
                    .resolve_secret_placeholders(&call.name, &call.arguments)
                    .unwrap_or_else(|_| call.arguments.clone());
                let hook_event_id = uuid::Uuid::new_v4().to_string();
                let hook_hint = action_message_hint(&resolved_args);
                self.fire_action_hook(
                    crate::hooks::HookTrigger::PreAction,
                    request_channel,
                    &call.name,
                    hook_hint.as_deref(),
                    None,
                    &hook_event_id,
                )
                .await;
                let llm_env = self.config.llm.app_env_vars();
                match crate::actions::app::app_deploy(
                    &self.config_dir,
                    &self.data_dir,
                    &resolved_args,
                    &self.app_registry,
                    &llm_env,
                    stream_tx.clone(),
                )
                .await
                {
                    Ok(result) => {
                        self.fire_action_hook(
                            crate::hooks::HookTrigger::PostAction,
                            request_channel,
                            &call.name,
                            hook_hint.as_deref(),
                            Some(&result),
                            &hook_event_id,
                        )
                        .await;
                        // Parse result to extract URL for a nice response
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result) {
                            if parsed
                                .get("status")
                                .and_then(|v| v.as_str())
                                .is_some_and(|s| s == "needs_secrets")
                            {
                                let title = parsed
                                    .get("title")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("App");
                                let app_id = parsed
                                    .get("app_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("app");
                                let missing = parsed
                                    .get("missing_env")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str())
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    })
                                    .unwrap_or_else(|| "unknown".to_string());
                                let missing_config = parsed
                                    .get("missing_config")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str())
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    })
                                    .unwrap_or_default();
                                let msg = format!(
                                    "App '{}' created but waiting on required inputs.\nMissing sensitive keys: {}{}\n\
                                     For sensitive keys ask user to run: set secret KEY=VALUE.\n\
                                     For non-sensitive config values, redeploy/restart with config.{{KEY}}=value.\n\
                                     Then restart app '{}'.",
                                    title,
                                    if missing.is_empty() { "none" } else { &missing },
                                    if missing_config.is_empty() { "".to_string() } else { format!("\nMissing config values: {}", missing_config) },
                                    app_id
                                );
                                if let Some(ref tx) = stream_tx {
                                    let _ = tx.try_send(StreamEvent::ToolResult {
                                        name: call.name.clone(),
                                        content: msg.clone(),
                                    });
                                }
                                results.push(msg);
                                continue;
                            }
                            if let Some(url) = parsed.get("url").and_then(|v| v.as_str()) {
                                let title = parsed
                                    .get("title")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("App");
                                let app_type = parsed
                                    .get("type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("static");
                                let app_id = parsed
                                    .get("app_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("app");
                                let access_key = parsed
                                    .get("access_key")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let url_with_key = if access_key.is_empty() {
                                    url.to_string()
                                } else {
                                    format!("{}?key={}", url, access_key)
                                };

                                let (preview_url, verified, verify_attempts, verify_detail) = self
                                    .validate_and_capture_app_preview(
                                        &url_with_key,
                                        app_id,
                                        stream_tx.as_ref(),
                                    )
                                    .await
                                    .unwrap_or_else(|e| {
                                        (None, false, 0, format!("Validation helper error: {}", e))
                                    });

                                if let Some(ref tx) = stream_tx {
                                    let _ = tx.try_send(StreamEvent::ToolResult {
                                        name: call.name.clone(),
                                        content: if verified {
                                            format!(
                                                "App deployed + validated: {} ({}) [{} attempt{}]",
                                                title,
                                                app_type,
                                                verify_attempts,
                                                if verify_attempts == 1 { "" } else { "s" }
                                            )
                                        } else {
                                            format!(
                                                "App deployed, validation incomplete: {} ({}) - {}",
                                                title, app_type, verify_detail
                                            )
                                        },
                                    });
                                }

                                let mut app_message = format!(
                                    "[APP_DEPLOYED]{}\n[/APP_DEPLOYED]\n*{} ({}) deployed and running*",
                                    url_with_key, title, app_type
                                );
                                app_message.push_str(&format!(
                                    "\nValidation: {} (attempts: {})",
                                    if verified { "PASSED" } else { "FAILED" },
                                    verify_attempts
                                ));
                                app_message
                                    .push_str(&format!("\nValidation note: {}", verify_detail));
                                if let Some(preview) = preview_url {
                                    app_message.push_str(&format!("\n![App Preview]({})", preview));
                                }
                                results.push(app_message);
                                continue;
                            }
                        }
                        if let Some(ref tx) = stream_tx {
                            let _ = tx.try_send(StreamEvent::ToolResult {
                                name: call.name.clone(),
                                content: sanitize_stream(&result),
                            });
                        }
                        results.push(result);
                    }
                    Err(e) => {
                        tracing::error!("App deployment error: {}", e);
                        self.fire_action_hook(
                            crate::hooks::HookTrigger::OnError,
                            request_channel,
                            &call.name,
                            hook_hint.as_deref(),
                            Some(&e.to_string()),
                            &hook_event_id,
                        )
                        .await;
                        let formatted = format!("Error deploying app: {}", e);
                        if let Some(ref tx) = stream_tx {
                            let _ = tx.try_send(StreamEvent::ToolResult {
                                name: call.name.clone(),
                                content: formatted.clone(),
                            });
                        }
                        results.push(formatted);
                    }
                }
                continue;
            }

            // Execute in sandbox (runtime will resolve secret placeholders at execution time)
            let call_message_hint = action_message_hint(&call.arguments);
            match self
                .execute_action_with_hooks(
                    &call.name,
                    &call.arguments,
                    request_channel,
                    call_message_hint.as_deref(),
                )
                .await
            {
                Ok(result) => {
                    let mut result = result;
                    if call.name.starts_with("mcp_") {
                        result = self.sanitize_mcp_output(&result);
                    }
                    // Special handling for schedule_task - actually create the task
                    if call.name == "schedule_task" && result.starts_with("Task scheduled:") {
                        if let Some(schedule_result) =
                            self.handle_schedule_task(&call.arguments).await
                        {
                            if let Some(ref tx) = stream_tx {
                                let _ = tx.try_send(StreamEvent::ToolResult {
                                    name: call.name.clone(),
                                    content: sanitize_stream(&schedule_result),
                                });
                            }
                            results.push(schedule_result);
                            continue;
                        }
                    }

                    // Special handling for watch - spawn background watcher
                    if call.name == "watch" && result.starts_with("Watch created:") {
                        if let Some(watch_result) = self.handle_watch(&call.arguments).await {
                            if let Some(ref tx) = stream_tx {
                                let _ = tx.try_send(StreamEvent::ToolResult {
                                    name: call.name.clone(),
                                    content: sanitize_stream(&watch_result),
                                });
                            }
                            results.push(watch_result);
                            continue;
                        }
                    }

                    // Format code_execute results with self-heal retry on errors
                    if call.name == "code_execute" {
                        let language = call
                            .arguments
                            .get("language")
                            .and_then(|v| v.as_str())
                            .unwrap_or("code")
                            .to_string();
                        let mut current_code = call
                            .arguments
                            .get("code")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let mut current_result = result.clone();
                        let mut current_args = call.arguments.clone();

                        // Self-heal loop: retry on execution errors
                        const MAX_SAME_ERROR_RETRIES: usize = 3;
                        const MAX_TOTAL_RETRIES: usize = 7;
                        let mut total_retries = 0usize;
                        let mut last_error_sig = String::new();
                        let mut same_error_count = 0usize;
                        let mut self_heal_stop_reason: Option<String> = None;
                        let mut self_heal_error_signatures: Vec<String> = Vec::new();
                        let code_signature = |code: &str| -> String {
                            let mut normalized = code
                                .lines()
                                .map(|line| line.trim())
                                .filter(|line| !line.is_empty())
                                .collect::<Vec<_>>()
                                .join("\n");
                            if normalized.len() > 4096 {
                                normalized.truncate(4096);
                            }
                            normalized
                        };
                        let mut seen_code_signatures: HashSet<String> = HashSet::new();
                        let initial_sig = code_signature(&current_code);
                        if !initial_sig.is_empty() {
                            seen_code_signatures.insert(initial_sig);
                        }

                        loop {
                            let parsed = match serde_json::from_str::<serde_json::Value>(
                                &current_result,
                            ) {
                                Ok(parsed) => parsed,
                                Err(_) => {
                                    if total_retries > 0 {
                                        self_heal_stop_reason = Some(
                                            "runtime response was not structured JSON; stopped auto-fix"
                                                .to_string(),
                                        );
                                    }
                                    break;
                                }
                            };
                            let exit_code =
                                parsed.get("exit_code").and_then(|v| v.as_i64()).unwrap_or(0);
                            let should_retry = exit_code != 0 && !current_code.trim().is_empty();

                            if !should_retry {
                                break;
                            }

                            if total_retries >= MAX_TOTAL_RETRIES {
                                self_heal_stop_reason =
                                    Some(format!("maximum attempts reached ({MAX_TOTAL_RETRIES})"));
                                break;
                            }

                            let error_text =
                                parsed.get("error").and_then(|v| v.as_str()).unwrap_or("");
                            let output_text =
                                parsed.get("output").and_then(|v| v.as_str()).unwrap_or("");
                            // Build error signature for same-error detection
                            let error_combined = format!("{}\n{}", error_text, output_text);
                            let error_sig = error_combined
                                .lines()
                                .take(5)
                                .collect::<Vec<_>>()
                                .join("\n");
                            if !error_sig.is_empty()
                                && !self_heal_error_signatures.iter().any(|s| s == &error_sig)
                                && self_heal_error_signatures.len() < 4
                            {
                                self_heal_error_signatures.push(error_sig.clone());
                            }

                            if error_sig == last_error_sig {
                                same_error_count += 1;
                                if same_error_count >= MAX_SAME_ERROR_RETRIES {
                                    tracing::warn!(
                                        "Self-heal: same error repeated {} times, giving up",
                                        same_error_count
                                    );
                                    self_heal_stop_reason = Some(format!(
                                        "same failure repeated {} times",
                                        same_error_count
                                    ));
                                    break;
                                }
                            } else {
                                same_error_count = 1;
                                last_error_sig = error_sig;
                            }

                            total_retries += 1;
                            tracing::info!("Self-heal: code execution failed (attempt {}/{}), asking LLM to fix", total_retries, MAX_TOTAL_RETRIES);

                            // Emit trace step
                            {
                                let mut trace = trace_ref.write().await;
                                trace.steps.push(ExecutionStep {
                                    icon: "[fix]".to_string(),
                                    title: format!(
                                        "Self-Heal: Fixing Code (attempt {})",
                                        total_retries
                                    ),
                                    detail: format!(
                                        "Error: {}",
                                        error_text.chars().take(100).collect::<String>()
                                    ),
                                    step_type: "thinking".to_string(),
                                    data: None,
                                    timestamp: chrono::Utc::now(),
                                    duration_ms: None,
                                });
                            }

                            // Ask LLM to fix the code
                            let fix_prompt = format!(
                                "The following {} code failed to execute. Fix the code and return ONLY the corrected code, no explanation.\n\n\
                                Code:\n```{}\n{}\n```\n\n\
                                Error output:\n```\n{}\n{}\n```\n\n\
                                Return only the fixed code, nothing else.",
                                language, language, current_code.trim(), error_text, output_text
                            );

                            let empty_actions: Vec<crate::actions::ActionDef> = Vec::new();
                            match self.llm.chat(
                                "You are a code fixer. Return ONLY the corrected code. No markdown fences, no explanations.",
                                &fix_prompt,
                                &[],
                                &empty_actions,
                            ).await {
                                Ok(fix_response) => {
                                    self.record_llm_usage(request_channel, "self_heal", &fix_response).await;
                                    // Extract code from response (strip markdown fences if present)
                                    let fixed = fix_response.content.trim().to_string();
                                    let fixed = if fixed.starts_with("```") {
                                        // Strip opening ```lang and closing ```
                                        let lines: Vec<&str> = fixed.lines().collect();
                                        let start = if lines.first().map_or(false, |l| l.starts_with("```")) { 1 } else { 0 };
                                        let end = if lines.last().map_or(false, |l| l.trim() == "```") { lines.len() - 1 } else { lines.len() };
                                        lines[start..end].join("\n")
                                    } else {
                                        fixed
                                    };
                                    let fixed_sig = code_signature(&fixed);
                                    let current_sig = code_signature(&current_code);
                                    if fixed_sig.is_empty() {
                                        tracing::warn!(
                                            "Self-heal: LLM returned empty code, giving up"
                                        );
                                        self_heal_stop_reason =
                                            Some("LLM returned empty patch".to_string());
                                        break;
                                    }
                                    if fixed_sig == current_sig {
                                        tracing::warn!("Self-heal: LLM returned identical code, giving up");
                                        self_heal_stop_reason =
                                            Some("LLM returned no meaningful code change".to_string());
                                        break;
                                    }
                                    if seen_code_signatures.contains(&fixed_sig) {
                                        tracing::warn!(
                                            "Self-heal: repeated patch detected, giving up"
                                        );
                                        self_heal_stop_reason = Some(
                                            "repeated patch detected (loop prevention)".to_string(),
                                        );
                                        break;
                                    }
                                    seen_code_signatures.insert(fixed_sig);

                                    current_code = fixed.clone();
                                    current_args["code"] = serde_json::Value::String(fixed);

                                    // Re-execute with fixed code
                                    let retry_hint = action_message_hint(&current_args);
                                    match self
                                        .execute_action_with_hooks(
                                            "code_execute",
                                            &current_args,
                                            request_channel,
                                            retry_hint.as_deref(),
                                        )
                                        .await
                                    {
                                        Ok(new_result) => {
                                            current_result = new_result;
                                        }
                                        Err(e) => {
                                            tracing::error!("Self-heal re-execution error: {}", e);
                                            self_heal_stop_reason = Some(format!(
                                                "re-execution failed: {}",
                                                safe_truncate(&e.to_string(), 180)
                                            ));
                                            break;
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Self-heal LLM call failed: {}", e);
                                    self_heal_stop_reason = Some(format!(
                                        "LLM fixer failed: {}",
                                        safe_truncate(&e.to_string(), 180)
                                    ));
                                    break;
                                }
                            }
                        }

                        // Format the final result (after retries or on first success)
                        let formatted = if let Ok(parsed) =
                            serde_json::from_str::<serde_json::Value>(&current_result)
                        {
                            let output =
                                parsed.get("output").and_then(|v| v.as_str()).unwrap_or("");
                            let error = parsed.get("error").and_then(|v| v.as_str());
                            let exit_code = parsed
                                .get("exit_code")
                                .and_then(|v| v.as_i64())
                                .unwrap_or(-1);
                            let files = parsed.get("files").and_then(|v| v.as_array());

                            let mut parts = Vec::new();

                            if total_retries > 0 {
                                let status = if exit_code == 0 {
                                    "fixed"
                                } else {
                                    "still failing"
                                };
                                parts.push(format!(
                                    "*Self-healed after {} attempt{} ({})*",
                                    total_retries,
                                    if total_retries == 1 { "" } else { "s" },
                                    status
                                ));
                                if exit_code != 0 {
                                    if let Some(reason) = &self_heal_stop_reason {
                                        parts.push(format!("**Self-heal stopped:** {}", reason));
                                    }
                                    if !self_heal_error_signatures.is_empty() {
                                        let signatures = self_heal_error_signatures
                                            .iter()
                                            .map(|s| format!("- `{}`", safe_truncate(s, 220)))
                                            .collect::<Vec<_>>()
                                            .join("\n");
                                        parts.push(format!(
                                            "**Observed failure signatures:**\n{}",
                                            signatures
                                        ));
                                    }
                                }
                            }

                            // Show the code with download link if available
                            if let Some(file_list) = &files {
                                let code_file = file_list
                                    .iter()
                                    .filter_map(|f| f.as_str())
                                    .find(|f| f.contains("code."));
                                if let Some(cf) = code_file {
                                    parts.push(format!(
                                        "```{}\n{}\n```\n[Download code]({})",
                                        language,
                                        current_code.trim(),
                                        cf
                                    ));
                                } else {
                                    parts.push(format!(
                                        "```{}\n{}\n```",
                                        language,
                                        current_code.trim()
                                    ));
                                }
                            } else {
                                parts.push(format!(
                                    "```{}\n{}\n```",
                                    language,
                                    current_code.trim()
                                ));
                            }

                            if !output.is_empty() {
                                parts.push(format!("**Output:**\n```\n{}\n```", output.trim()));
                            }

                            if let Some(err) = error {
                                if !err.is_empty() {
                                    parts.push(format!("**Errors:**\n```\n{}\n```", err.trim()));
                                }
                            }

                            if exit_code != 0 {
                                parts.push(format!("Exit code: {}", exit_code));
                            }

                            if let Some(file_list) = files {
                                let output_files: Vec<&str> = file_list
                                    .iter()
                                    .filter_map(|f| f.as_str())
                                    .filter(|f| !f.contains("code."))
                                    .collect();
                                if !output_files.is_empty() {
                                    let mut file_parts = Vec::new();
                                    for file_path in &output_files {
                                        let filename =
                                            file_path.rsplit('/').next().unwrap_or(file_path);
                                        let ext = filename
                                            .rsplit('.')
                                            .next()
                                            .unwrap_or("")
                                            .to_lowercase();
                                        let image_exts =
                                            ["png", "jpg", "jpeg", "gif", "svg", "webp", "bmp"];
                                        if image_exts.contains(&ext.as_str()) {
                                            file_parts
                                                .push(format!("![{}]({})", filename, file_path));
                                        } else {
                                            file_parts.push(format!(
                                                "[Download {}]({})",
                                                filename, file_path
                                            ));
                                        }
                                    }
                                    parts.push(format!(
                                        "**Generated Files:**\n{}",
                                        file_parts.join("\n")
                                    ));
                                }
                            }

                            parts.join("\n\n")
                        } else {
                            let mut prefix = String::new();
                            if total_retries > 0 {
                                let mut line = format!(
                                    "*Self-healed after {} attempt{} (still failing)*",
                                    total_retries,
                                    if total_retries == 1 { "" } else { "s" }
                                );
                                if let Some(reason) = &self_heal_stop_reason {
                                    line.push_str(&format!("\n**Self-heal stopped:** {}", reason));
                                }
                                prefix.push_str(&line);
                                prefix.push_str("\n\n");
                            }
                            format!(
                                "{}```{}\n{}\n```\n\n{}",
                                prefix,
                                language,
                                current_code.trim(),
                                current_result
                            )
                        };

                        results.push(formatted);
                        continue;
                    }

                    // Format video_generate results with inline player + download
                    if call.name == "video_generate" {
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&result) {
                            if let Some(url) = parsed.get("url").and_then(|v| v.as_str()) {
                                let duration = parsed
                                    .get("duration_seconds")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let resolution = parsed
                                    .get("resolution")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let size = parsed
                                    .get("file_size_bytes")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0);
                                let output_id = parsed
                                    .get("output_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let filename = parsed
                                    .get("filename")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                let size_mb = size as f64 / 1_048_576.0;
                                let rendered_url = absolutize_url(url);
                                let mut delivered_to_channel = false;
                                let caption = format!(
                                    "{}s video, {}, {:.1}MB",
                                    duration, resolution, size_mb
                                );

                                if matches!(request_channel, "telegram" | "whatsapp")
                                    && !output_id.is_empty()
                                    && !filename.is_empty()
                                {
                                    let output_path = self
                                        .data_dir
                                        .join("outputs")
                                        .join(output_id)
                                        .join(filename);
                                    match tokio::fs::read(&output_path).await {
                                        Ok(video_bytes) => {
                                            match crate::channels::send_video_to_channel(
                                                self,
                                                request_channel,
                                                &video_bytes,
                                                &caption,
                                                Some(&rendered_url),
                                            )
                                            .await
                                            {
                                                Ok(_) => {
                                                    delivered_to_channel = true;
                                                }
                                                Err(e) => {
                                                    tracing::warn!(
                                                        "Failed to send generated video to {}: {}",
                                                        request_channel,
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tracing::warn!(
                                                "Failed reading rendered video file {}: {}",
                                                output_path.display(),
                                                e
                                            );
                                        }
                                    }
                                }

                                let formatted = if delivered_to_channel {
                                    format!("Video sent to this chat.\nDownload: {}", rendered_url)
                                } else if matches!(request_channel, "telegram" | "whatsapp") {
                                    format!(
                                        "Video generated ({}s, {:.1}MB): {}",
                                        duration, size_mb, rendered_url
                                    )
                                } else {
                                    format!(
                                        "[VIDEO_RESULT]{}\n[/VIDEO_RESULT]\n*{}s video, {}, {:.1}MB*",
                                        rendered_url, duration, resolution, size_mb
                                    )
                                };
                                if let Some(ref tx) = stream_tx {
                                    let _ = tx.try_send(StreamEvent::ToolResult {
                                        name: call.name.clone(),
                                        content: format!(
                                            "Video generated ({}s, {:.1}MB)",
                                            duration, size_mb
                                        ),
                                    });
                                }
                                results.push(formatted);
                                continue;
                            }
                        }
                        if let Some(ref tx) = stream_tx {
                            let _ = tx.try_send(StreamEvent::ToolResult {
                                name: call.name.clone(),
                                content: sanitize_stream(&result),
                            });
                        }
                        results.push(result);
                        continue;
                    }

                    // Format gmail_scan results with LLM classification + summary
                    if call.name == "gmail_scan" {
                        let email_format_hint = {
                            let profile = self.user_profile.read().await;
                            profile.email_format.clone().unwrap_or_default()
                        };
                        let format_extra = if email_format_hint.is_empty() {
                            String::new()
                        } else {
                            format!("\nUser preference: {}", email_format_hint)
                        };

                        let format_prompt = format!(
                            "Here are raw email results from Gmail. Classify, summarize, and format them.\n\
                            Rules:\n\
                            - Group into categories with **bold** headers: Action Needed, Security Alerts, Receipts & Orders, Newsletters & Promotions, Other\n\
                            - Skip empty categories\n\
                            - For each email: show sender name (not full email address), subject, and a brief one-line summary/gist\n\
                            - Flag anything time-sensitive or requiring action\n\
                            - Use markdown: **bold** for headers, bullet points for items\n\
                            - Be concise - no raw headers, no IDs, no label dumps\n\
                            {}\n\n\
                            Raw email data:\n{}",
                            format_extra, result
                        );

                        let empty_actions: Vec<crate::actions::ActionDef> = Vec::new();
                        match self.llm.chat(
                            "You are a concise email assistant. Format email summaries with clear categorization. Use markdown.",
                            &format_prompt,
                            &[],
                            &empty_actions,
                        ).await {
                            Ok(formatted) => {
                                self.record_llm_usage(request_channel, "gmail_format", &formatted).await;
                                if let Some(ref tx) = stream_tx {
                                    let _ = tx.try_send(StreamEvent::ToolResult {
                                        name: call.name.clone(),
                                        content: "Gmail scan summarized".to_string(),
                                    });
                                }
                                results.push(formatted.content);
                            }
                            Err(e) => {
                                tracing::warn!("Gmail format LLM pass failed, using raw: {}", e);
                                if let Some(ref tx) = stream_tx {
                                    let _ = tx.try_send(StreamEvent::ToolResult {
                                        name: call.name.clone(),
                                        content: "Gmail scan returned raw results".to_string(),
                                    });
                                }
                                results.push(result);
                            }
                        }
                        continue;
                    }

                    if let Some(payload) = parse_workflow_missing_inputs_marker(&result) {
                        let prompt = Self::format_missing_inputs_prompt(&payload);
                        if let Some(ref tx) = stream_tx {
                            let _ = tx.try_send(StreamEvent::ToolResult {
                                name: call.name.clone(),
                                content: prompt.clone(),
                            });
                        }
                        results.push(prompt);
                        continue;
                    }

                    // Check if this is a workflow action that needs LLM orchestration
                    if let Some((action_name, user_query)) = parse_workflow_action_marker(&result) {
                        match self
                            .execute_workflow_marker_action(&action_name, &user_query)
                            .await
                        {
                            Ok(llm_result) => {
                                if let Some(ref tx) = stream_tx {
                                    let _ = tx.try_send(StreamEvent::ToolResult {
                                        name: call.name.clone(),
                                        content: format!("Workflow '{}' completed", action_name),
                                    });
                                }
                                results.push(llm_result);
                            }
                            Err(e) => {
                                tracing::error!("Workflow action execution error: {}", e);
                                let formatted =
                                    format!("Error executing workflow '{}': {}", action_name, e);
                                if let Some(ref tx) = stream_tx {
                                    let _ = tx.try_send(StreamEvent::ToolResult {
                                        name: call.name.clone(),
                                        content: formatted.clone(),
                                    });
                                }
                                results.push(formatted);
                            }
                        }
                        continue;
                    }

                    if let Some(ref tx) = stream_tx {
                        let _ = tx.try_send(StreamEvent::ToolResult {
                            name: call.name.clone(),
                            content: sanitize_stream(&result),
                        });
                    }
                    results.push(result);
                }
                Err(e) => {
                    tracing::error!("Action execution error: {}", e);
                    let formatted = format!("Error executing '{}': {}", call.name, e);
                    if let Some(ref tx) = stream_tx {
                        let _ = tx.try_send(StreamEvent::ToolResult {
                            name: call.name.clone(),
                            content: formatted.clone(),
                        });
                    }
                    results.push(formatted);
                }
            }
        }

        // If there's content plus tool results, combine them
        if response.content.is_empty() {
            Ok(results.join("\n"))
        } else {
            Ok(format!("{}\n\n{}", response.content, results.join("\n")))
        }
    }


}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(id: &str, name: &str, arguments: serde_json::Value) -> crate::core::llm::ToolCall {
        crate::core::llm::ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments,
        }
    }

    #[test]
    fn tool_call_signature_ignores_object_key_order() {
        let a = call(
            "1",
            "app_deploy",
            json!({
                "files": {"index.html": "<h1>ok</h1>"},
                "title": "demo",
                "config": {"a": 1, "b": 2}
            }),
        );
        let b = call(
            "2",
            "app_deploy",
            json!({
                "config": {"b": 2, "a": 1},
                "title": "demo",
                "files": {"index.html": "<h1>ok</h1>"}
            }),
        );

        assert_eq!(Agent::tool_call_signature(&a), Agent::tool_call_signature(&b));
    }

    #[test]
    fn tool_call_signature_preserves_array_order() {
        let a = call("1", "code_execute", json!({ "args": [1, 2, 3] }));
        let b = call("2", "code_execute", json!({ "args": [3, 2, 1] }));

        assert_ne!(Agent::tool_call_signature(&a), Agent::tool_call_signature(&b));
    }
}

