use super::*;

const ROUTING_COMPLEXITY_POLICY_KEY: &str = "routing_complexity_policy_v1";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RoutingComplexityPolicy {
    complex_indicators: Vec<String>,
    medium_indicators: Vec<String>,
    long_question_word_threshold: usize,
    long_message_word_threshold: usize,
    multi_sentence_threshold: usize,
}

impl Default for RoutingComplexityPolicy {
    fn default() -> Self {
        Self {
            complex_indicators: vec![
                "research".to_string(),
                "investigate".to_string(),
                "analyze and".to_string(),
                "compare and".to_string(),
                "write a report".to_string(),
                "write an article".to_string(),
                "comprehensive".to_string(),
                "step by step".to_string(),
                "multiple".to_string(),
                "all of".to_string(),
                "each of".to_string(),
            ],
            medium_indicators: vec![
                "explain".to_string(),
                "why".to_string(),
                "how does".to_string(),
                "what is the difference".to_string(),
                "should i".to_string(),
                "which is better".to_string(),
                "pros and cons".to_string(),
                "analyze".to_string(),
                "evaluate".to_string(),
                "recommend".to_string(),
                "suggest".to_string(),
                "help me understand".to_string(),
                "clarify".to_string(),
                "create a".to_string(),
                "build a".to_string(),
                "develop".to_string(),
                "implement".to_string(),
                "design".to_string(),
                "make a".to_string(),
                "deploy".to_string(),
                "generate".to_string(),
                "send".to_string(),
                "check".to_string(),
                "fix".to_string(),
            ],
            long_question_word_threshold: 50,
            long_message_word_threshold: 30,
            multi_sentence_threshold: 3,
        }
    }
}

impl Agent {
    fn apply_routing_complexity_policy_override(
        policy: &mut RoutingComplexityPolicy,
        raw: &serde_json::Value,
    ) {
        let Some(obj) = raw.as_object() else {
            return;
        };

        if let Some(v) = obj.get("complex_indicators").and_then(|v| v.as_array()) {
            policy.complex_indicators = v
                .iter()
                .filter_map(|item| item.as_str())
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
        }
        if let Some(v) = obj.get("medium_indicators").and_then(|v| v.as_array()) {
            policy.medium_indicators = v
                .iter()
                .filter_map(|item| item.as_str())
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
        }
        if let Some(v) = obj
            .get("long_question_word_threshold")
            .and_then(|v| v.as_u64())
        {
            policy.long_question_word_threshold = v.clamp(5, 1000) as usize;
        }
        if let Some(v) = obj
            .get("long_message_word_threshold")
            .and_then(|v| v.as_u64())
        {
            policy.long_message_word_threshold = v.clamp(5, 1000) as usize;
        }
        if let Some(v) = obj.get("multi_sentence_threshold").and_then(|v| v.as_u64()) {
            policy.multi_sentence_threshold = v.clamp(1, 50) as usize;
        }
    }

    async fn load_routing_complexity_policy(&self) -> RoutingComplexityPolicy {
        let mut policy = RoutingComplexityPolicy::default();

        if let Ok(raw_env) = std::env::var("AGENTARK_ROUTING_COMPLEXITY_POLICY_JSON") {
            match serde_json::from_str::<serde_json::Value>(&raw_env) {
                Ok(value) => Self::apply_routing_complexity_policy_override(&mut policy, &value),
                Err(e) => tracing::warn!(
                    "Invalid AGENTARK_ROUTING_COMPLEXITY_POLICY_JSON ignored: {}",
                    e
                ),
            }
        }

        if let Ok(Some(raw)) = self.storage.get(ROUTING_COMPLEXITY_POLICY_KEY).await {
            match serde_json::from_slice::<serde_json::Value>(&raw) {
                Ok(value) => Self::apply_routing_complexity_policy_override(&mut policy, &value),
                Err(e) => tracing::warn!(
                    "Invalid routing complexity policy in storage ignored: {}",
                    e
                ),
            }
        }

        policy
    }

    /// Select the best model role based on message content and complexity
    pub(crate) fn select_model_role(&self, message: &str, complexity: &QueryComplexity) -> ModelRole {
        if !self.config.model_pool.smart_routing {
            return ModelRole::Primary;
        }
        let msg_lower = message.to_lowercase();
        let tokens = tokenize_lower(message);
        let has_role = |role: ModelRole| {
            self.model_pool
                .values()
                .any(|(s, _)| s.role == role && s.enabled)
        };

        if (msg_lower.contains("deep research")
            || msg_lower.contains("research in depth")
            || msg_lower.starts_with("[deep_research]"))
            && has_role(ModelRole::Research)
        {
            return ModelRole::Research;
        }

        let research_terms = [
            "research",
            "paper",
            "literature",
            "survey",
            "benchmark",
            "arxiv",
        ];
        let research_hits = tokens
            .iter()
            .filter(|t| research_terms.iter().any(|k| t.contains(k)))
            .count();
        if has_role(ModelRole::Research)
            && (research_hits >= 2
                || (research_hits >= 1 && matches!(complexity, QueryComplexity::Complex)))
        {
            return ModelRole::Research;
        }

        let code_terms = [
            "code",
            "function",
            "class",
            "bug",
            "debug",
            "refactor",
            "python",
            "javascript",
            "rust",
            "typescript",
            "sql",
            "regex",
            "algorithm",
            "compile",
            "stacktrace",
            "exception",
        ];
        let code_hits = tokens
            .iter()
            .filter(|t| code_terms.iter().any(|k| t.contains(k)))
            .count();
        let code_syntax_signal = message.contains("```")
            || message.contains("fn ")
            || message.contains("def ")
            || message.contains("SELECT ")
            || message.contains("select ");
        if has_role(ModelRole::Code) && (code_hits >= 2 || code_syntax_signal) {
            return ModelRole::Code;
        }

        match complexity {
            QueryComplexity::Simple => {
                if has_role(ModelRole::Fast) {
                    ModelRole::Fast
                } else {
                    ModelRole::Primary
                }
            }
            _ => ModelRole::Primary,
        }
    }

    /// LLM-based routing: decide if we need sub-agents and what kind
    pub(crate) async fn route_query(
        &self,
        message: &str,
        actions: &[crate::actions::ActionDef],
    ) -> crate::core::task_router::RoutingDecision {
        let router_llm = self.llm_for_role(&ModelRole::Fast);

        let specialist_desc = if self.config.swarm.specialists.is_empty() {
            "None configured.".to_string()
        } else {
            self.config
                .swarm
                .specialists
                .iter()
                .filter(|s| s.enabled)
                .map(|s| {
                    format!(
                        "- {} ({:?}): {}",
                        s.name,
                        s.agent_type,
                        s.capabilities
                            .iter()
                            .map(|c| c.description.clone())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        let routing_prompt = format!(
            r#"Analyze this task and decide the execution strategy. Respond with ONLY valid JSON.

Available agent types for sub-agents: Researcher, Coder, Analyst, Writer, Validator, Planner
Available model roles: Primary, Fast, Code, Research
Custom specialists: {specialists}
{router_policy}

Rules:
- "needs_delegation": true ONLY for pure analysis/research tasks that truly need multiple independent agents.
- For execution tasks (build/create/make/deploy/run/send/check/fix), prefer direct execution:
  needs_delegation=false unless there is explicit parallel decomposition.
- If the request is ambiguous or underspecified, set should_clarify=true.
- Any retry/repair strategy MUST define a hard maximum attempts cap.
- confidence is a number in [0,1]. Use >=0.90 only when intent is very clear.
- depends_on: index of a sub-agent whose result this one needs (use [] if independent/parallel)

JSON format:
{{"needs_delegation": false, "complexity": "simple", "sub_agents": [], "reasoning": "brief why", "confidence": 0.90, "should_clarify": false, "clarification_question": null}}

OR for delegation:
{{"needs_delegation": true, "complexity": "complex", "sub_agents": [{{"agent_type": "Researcher", "task": "specific task", "preferred_model_role": null, "depends_on": []}}], "reasoning": "brief why", "confidence": 0.78, "should_clarify": false, "clarification_question": null}}

If should_clarify=true, provide a short concrete question in clarification_question.

Task: {message}"#,
            specialists = specialist_desc,
            router_policy = crate::core::prompt_policy::router_policy_v2_block(),
            message = message
        );

        let empty_actions: Vec<crate::actions::ActionDef> = Vec::new();
        match router_llm
            .chat(
                "You are a task router. Follow Router Policy v2. Output only valid JSON. No markdown, no explanation.",
                &routing_prompt,
                &[],
                &empty_actions,
            )
            .await
        {
            Ok(response) => {
                let content = response.content.trim();
                let json_str = if content.starts_with("```") {
                    content
                        .lines()
                        .skip(1)
                        .take_while(|l| !l.starts_with("```"))
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    content.to_string()
                };

                match serde_json::from_str::<crate::core::task_router::RoutingDecision>(&json_str)
                {
                    Ok(mut decision) => {
                        if !(0.0..=1.0).contains(&decision.confidence) || decision.confidence <= 0.0
                        {
                            decision.confidence = if decision.needs_delegation { 0.75 } else { 0.65 };
                        }

                        let app_intent = has_action_intent_default(message, actions, "app_deploy");
                        if has_execution_intent(message, actions) && app_intent {
                            decision.needs_delegation = false;
                            decision.complexity = QueryComplexity::Simple;
                            decision.sub_agents.clear();
                            decision.reasoning = format!(
                                "{} | App/deploy request forced to direct execution path",
                                decision.reasoning
                            );
                            decision.confidence = decision.confidence.max(0.90);
                        }

                        if has_execution_intent(message, actions)
                            && decision.needs_delegation
                            && decision.confidence < 0.90
                        {
                            decision.needs_delegation = false;
                            decision.complexity = QueryComplexity::Simple;
                            decision.sub_agents.clear();
                            decision.reasoning = format!(
                                "{} | Execution task routed to direct tool path (confidence below 0.90)",
                                decision.reasoning
                            );
                        }

                        if has_execution_intent(message, actions) && decision.confidence < 0.90 {
                            // Skip clarification when a single action matches strongly —
                            // the user's intent is clear enough to execute directly.
                            let best_score = best_execution_intent_score(message, actions);
                            if best_score < 0.80 {
                                let ambiguous = is_ambiguous_user_request(message, actions);
                                decision.should_clarify = true;
                                if decision.clarification_question.is_none() {
                                    decision.clarification_question = Some(if ambiguous {
                                        "I can do that. What exactly should I build and should I deploy it as a live app link?"
                                            .to_string()
                                    } else {
                                        "I can execute that now. Confirm the exact output you want me to deliver."
                                            .to_string()
                                    });
                                }
                            } else {
                                // High-confidence action match — boost routing confidence
                                // so the LLM proceeds without asking.
                                decision.confidence = decision.confidence.max(0.92);
                                decision.should_clarify = false;
                                decision.clarification_question = None;
                            }
                        }

                        decision
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to parse routing JSON, falling back to keyword: {}",
                            e
                        );
                        self.classify_complexity_fallback(message, actions).await
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Routing LLM call failed, falling back to keyword: {}", e);
                self.classify_complexity_fallback(message, actions).await
            }
        }
    }

    /// Fallback: keyword-based complexity classification (used when LLM routing fails)
    pub(crate) async fn classify_complexity_fallback(
        &self,
        message: &str,
        actions: &[crate::actions::ActionDef],
    ) -> crate::core::task_router::RoutingDecision {
        let mut complexity = self.classify_complexity(message).await;
        let execution_intent = has_execution_intent(message, actions);
        if execution_intent && matches!(complexity, QueryComplexity::Complex) {
            complexity = QueryComplexity::Medium;
        }

        let should_clarify = execution_intent;
        crate::core::task_router::RoutingDecision {
            needs_delegation: matches!(complexity, QueryComplexity::Complex) && !execution_intent,
            complexity,
            sub_agents: vec![],
            reasoning: "Fallback keyword classification".to_string(),
            confidence: if should_clarify { 0.45 } else { 0.60 },
            should_clarify,
            clarification_question: if should_clarify {
                Some("I can do that. What exactly do you want me to execute right now?".to_string())
            } else {
                None
            },
        }
    }

    /// Classify query complexity for routing (keyword-based, used as fallback)
    pub(crate) async fn classify_complexity(&self, message: &str) -> QueryComplexity {
        let msg_lower = message.to_lowercase();
        let word_count = message.split_whitespace().count();

        let policy = self.load_routing_complexity_policy().await;

        for indicator in &policy.complex_indicators {
            if !indicator.is_empty() && msg_lower.contains(indicator) {
                return QueryComplexity::Complex;
            }
        }
        if word_count > policy.long_question_word_threshold && msg_lower.contains('?') {
            return QueryComplexity::Complex;
        }
        for indicator in &policy.medium_indicators {
            if !indicator.is_empty() && msg_lower.contains(indicator) {
                return QueryComplexity::Medium;
            }
        }
        let sentence_count = message.matches('.').count() + message.matches('?').count();
        if sentence_count >= policy.multi_sentence_threshold
            || word_count > policy.long_message_word_threshold
        {
            return QueryComplexity::Medium;
        }
        QueryComplexity::Simple
    }
}
