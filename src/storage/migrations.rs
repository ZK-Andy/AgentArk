use anyhow::Result;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};

const MIGRATION_LOCK_ID: i64 = 870_334_221;

fn backend_bind_sql(backend: DbBackend, sql: &str) -> String {
    if backend != DbBackend::Postgres {
        return sql.to_string();
    }

    let mut out = String::with_capacity(sql.len() + 16);
    let mut index = 1_u32;
    let mut chars = sql.chars().peekable();
    let mut in_single_quote = false;

    while let Some(ch) = chars.next() {
        if ch == '\'' {
            out.push(ch);
            if in_single_quote && chars.peek() == Some(&'\'') {
                out.push(chars.next().unwrap_or('\''));
                continue;
            }
            in_single_quote = !in_single_quote;
            continue;
        }

        if ch == '?' && !in_single_quote {
            out.push('$');
            out.push_str(&index.to_string());
            index += 1;
        } else {
            out.push(ch);
        }
    }

    out
}

fn statement_with_values(
    backend: DbBackend,
    sql: impl Into<String>,
    values: Vec<sea_orm::Value>,
) -> Statement {
    let sql = sql.into();
    Statement::from_sql_and_values(backend, backend_bind_sql(backend, &sql), values)
}

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "postgres_baseline",
        sql: r#"
CREATE TABLE IF NOT EXISTS kv_store (
    key TEXT PRIMARY KEY,
    value BYTEA NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS episodes (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    context TEXT NOT NULL,
    embedding BYTEA,
    timestamp TEXT NOT NULL,
    consolidated BOOLEAN NOT NULL DEFAULT FALSE,
    importance REAL NOT NULL DEFAULT 0.5,
    last_accessed TEXT,
    access_count INTEGER NOT NULL DEFAULT 0,
    project_id TEXT
);

CREATE TABLE IF NOT EXISTS semantic_facts (
    id TEXT PRIMARY KEY,
    fact TEXT NOT NULL,
    confidence REAL NOT NULL,
    sources TEXT NOT NULL,
    embedding BYTEA,
    created_at TEXT NOT NULL,
    project_id TEXT
);

CREATE TABLE IF NOT EXISTS actions (
    name TEXT PRIMARY KEY,
    version TEXT NOT NULL,
    wasm_hash TEXT,
    source TEXT NOT NULL,
    success_rate REAL NOT NULL DEFAULT 1.0,
    execution_count INTEGER NOT NULL DEFAULT 0,
    last_used TEXT
);

CREATE TABLE IF NOT EXISTS execution_proofs (
    id TEXT PRIMARY KEY,
    action_hash TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    output_hash TEXT NOT NULL,
    prev_hash TEXT,
    timestamp TEXT NOT NULL,
    signature TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS execution_traces (
    id TEXT PRIMARY KEY,
    message TEXT NOT NULL,
    channel TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    duration_ms INTEGER,
    step_count INTEGER NOT NULL DEFAULT 0,
    steps_json TEXT NOT NULL,
    response TEXT,
    proof_id TEXT REFERENCES execution_proofs(id) ON DELETE SET NULL,
    model TEXT,
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    cost_usd DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    complexity TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY,
    description TEXT NOT NULL,
    action TEXT NOT NULL,
    arguments TEXT NOT NULL,
    approval TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    scheduled_for TEXT,
    cron TEXT,
    result TEXT,
    proof_id TEXT REFERENCES execution_proofs(id) ON DELETE SET NULL,
    priority DOUBLE PRECISION,
    urgency DOUBLE PRECISION,
    importance DOUBLE PRECISION,
    eisenhower_quadrant INTEGER,
    lease_owner TEXT,
    lease_expires_at TEXT,
    lease_version INTEGER NOT NULL DEFAULT 0,
    next_retry_at TEXT,
    last_run_id TEXT,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    CHECK (char_length(trim(action)) > 0),
    CHECK (eisenhower_quadrant IS NULL OR eisenhower_quadrant BETWEEN 1 AND 4)
);

CREATE TABLE IF NOT EXISTS swarm_agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    agent_type TEXT NOT NULL,
    llm_provider TEXT NOT NULL,
    capabilities TEXT NOT NULL,
    system_prompt TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS swarm_delegations (
    id TEXT PRIMARY KEY,
    parent_task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    agent_id TEXT NOT NULL REFERENCES swarm_agents(id) ON DELETE CASCADE,
    task_description TEXT NOT NULL,
    result TEXT,
    success BOOLEAN NOT NULL DEFAULT FALSE,
    confidence REAL,
    execution_time_ms INTEGER,
    created_at TEXT NOT NULL,
    completed_at TEXT
);

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    system_prompt TEXT,
    personality TEXT,
    tools_filter TEXT,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS conversations (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    channel TEXT NOT NULL,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    message_count INTEGER NOT NULL DEFAULT 0,
    archived BOOLEAN NOT NULL DEFAULT FALSE,
    starred BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    model_used TEXT,
    trace_id TEXT REFERENCES execution_traces(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS documents (
    id TEXT PRIMARY KEY,
    filename TEXT NOT NULL,
    content_type TEXT NOT NULL,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    chunk_count INTEGER NOT NULL DEFAULT 0,
    file_size BIGINT NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS document_chunks (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    content TEXT NOT NULL,
    embedding BYTEA
);

CREATE TABLE IF NOT EXISTS notifications (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    level TEXT NOT NULL DEFAULT 'info',
    source TEXT NOT NULL DEFAULT '',
    read BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS approval_log (
    id TEXT PRIMARY KEY,
    action_name TEXT NOT NULL,
    arguments TEXT NOT NULL,
    rule_name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    requested_at TEXT NOT NULL,
    resolved_at TEXT,
    resolved_by TEXT
);

CREATE TABLE IF NOT EXISTS automation_runs (
    id TEXT PRIMARY KEY,
    automation_id TEXT NOT NULL,
    started_at TEXT NOT NULL,
    payload TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS automation_supervisor_states (
    automation_id TEXT PRIMARY KEY,
    updated_at TEXT NOT NULL,
    payload TEXT NOT NULL,
    lease_owner TEXT,
    lease_expires_at TEXT,
    lease_version INTEGER NOT NULL DEFAULT 0,
    next_retry_at TEXT,
    last_run_id TEXT,
    consecutive_failures INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS watchers (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    payload TEXT NOT NULL,
    lease_owner TEXT,
    lease_expires_at TEXT,
    lease_version INTEGER NOT NULL DEFAULT 0,
    next_retry_at TEXT,
    last_run_id TEXT,
    consecutive_failures INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS expenses (
    id TEXT PRIMARY KEY,
    amount DOUBLE PRECISION NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    category TEXT NOT NULL,
    description TEXT NOT NULL,
    date TEXT NOT NULL,
    payment_method TEXT,
    vendor TEXT,
    tags TEXT,
    split_with TEXT,
    receipt_path TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS security_logs (
    id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    severity TEXT NOT NULL,
    message TEXT NOT NULL,
    source TEXT,
    count INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS operational_logs (
    id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    trace_id TEXT REFERENCES execution_traces(id) ON DELETE SET NULL,
    conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
    channel TEXT NOT NULL DEFAULT '',
    event_type TEXT NOT NULL,
    success BOOLEAN NOT NULL DEFAULT FALSE,
    outcome TEXT NOT NULL DEFAULT '',
    tool_name TEXT,
    latency_ms INTEGER,
    arguments TEXT,
    payload TEXT,
    strategy_version TEXT,
    policy_version TEXT,
    prompt_version TEXT,
    model_slot TEXT
);

CREATE TABLE IF NOT EXISTS llm_usage (
    id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    channel TEXT NOT NULL,
    purpose TEXT NOT NULL DEFAULT '',
    prompt_tokens INTEGER NOT NULL,
    completion_tokens INTEGER NOT NULL,
    total_tokens INTEGER NOT NULL,
    estimated BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TABLE IF NOT EXISTS user_preferences (
    id TEXT PRIMARY KEY,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 0.8,
    source TEXT,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS user_data_items (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    url TEXT,
    source_channel TEXT,
    conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    pinned BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS knowledge_items (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    source TEXT,
    url TEXT,
    tags TEXT,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS execution_runs (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    request_id TEXT,
    status TEXT NOT NULL,
    current_stage TEXT NOT NULL,
    lease_owner TEXT,
    lease_expires_at TEXT,
    attempt INTEGER NOT NULL DEFAULT 0,
    deadline_at TEXT,
    cancellation_requested BOOLEAN NOT NULL DEFAULT FALSE,
    degradation TEXT NOT NULL DEFAULT '[]',
    last_error TEXT,
    result_summary TEXT,
    trace_id TEXT,
    conversation_id TEXT,
    channel TEXT,
    request_message TEXT,
    attempted_models TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS run_checkpoints (
    id BIGSERIAL PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES execution_runs(id) ON DELETE CASCADE,
    sequence_no INTEGER NOT NULL,
    stage TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE (run_id, sequence_no)
);

CREATE TABLE IF NOT EXISTS tool_attempts (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES execution_runs(id) ON DELETE CASCADE,
    sequence_no INTEGER NOT NULL,
    tool_name TEXT NOT NULL,
    status TEXT NOT NULL,
    failure_class TEXT,
    retryable BOOLEAN NOT NULL DEFAULT FALSE,
    side_effect_level TEXT NOT NULL DEFAULT 'unknown',
    idempotency_key TEXT,
    arguments_json TEXT NOT NULL DEFAULT '{}',
    output_json TEXT NOT NULL DEFAULT '{}',
    started_at TEXT NOT NULL,
    completed_at TEXT,
    error_text TEXT,
    UNIQUE (run_id, sequence_no)
);

CREATE INDEX IF NOT EXISTS idx_episodes_timestamp ON episodes(timestamp);
CREATE INDEX IF NOT EXISTS idx_episodes_project_id ON episodes(project_id);
CREATE INDEX IF NOT EXISTS idx_proofs_timestamp ON execution_proofs(timestamp);
CREATE INDEX IF NOT EXISTS idx_execution_traces_created ON execution_traces(created_at);
CREATE INDEX IF NOT EXISTS idx_execution_traces_started ON execution_traces(started_at);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
CREATE INDEX IF NOT EXISTS idx_tasks_scheduled_for ON tasks(scheduled_for);
CREATE INDEX IF NOT EXISTS idx_tasks_status_scheduled ON tasks(status, scheduled_for);
CREATE INDEX IF NOT EXISTS idx_tasks_created_at ON tasks(created_at);
CREATE INDEX IF NOT EXISTS idx_tasks_lease_expires_at ON tasks(lease_expires_at);
CREATE INDEX IF NOT EXISTS idx_tasks_next_retry_at ON tasks(next_retry_at);
CREATE INDEX IF NOT EXISTS idx_swarm_delegations_agent ON swarm_delegations(agent_id);
CREATE INDEX IF NOT EXISTS idx_messages_conversation ON messages(conversation_id);
CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON messages(timestamp);
CREATE INDEX IF NOT EXISTS idx_messages_role_timestamp ON messages(role, timestamp);
CREATE INDEX IF NOT EXISTS idx_conversations_updated ON conversations(updated_at);
CREATE INDEX IF NOT EXISTS idx_conversations_project ON conversations(project_id);
CREATE INDEX IF NOT EXISTS idx_conversations_starred_updated ON conversations(starred, updated_at);
CREATE INDEX IF NOT EXISTS idx_documents_project ON documents(project_id);
CREATE INDEX IF NOT EXISTS idx_document_chunks_doc ON document_chunks(document_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_document_chunks_doc_chunk ON document_chunks(document_id, chunk_index);
CREATE INDEX IF NOT EXISTS idx_notifications_created ON notifications(created_at);
CREATE INDEX IF NOT EXISTS idx_approval_log_status ON approval_log(status);
CREATE INDEX IF NOT EXISTS idx_approval_log_requested ON approval_log(requested_at);
CREATE INDEX IF NOT EXISTS idx_automation_runs_started ON automation_runs(started_at);
CREATE INDEX IF NOT EXISTS idx_automation_runs_automation_id ON automation_runs(automation_id);
CREATE INDEX IF NOT EXISTS idx_watchers_status ON watchers(status);
CREATE INDEX IF NOT EXISTS idx_watchers_created ON watchers(created_at);
CREATE INDEX IF NOT EXISTS idx_watchers_next_retry_at ON watchers(next_retry_at);
CREATE INDEX IF NOT EXISTS idx_watchers_lease_expires_at ON watchers(lease_expires_at);
CREATE INDEX IF NOT EXISTS idx_facts_project_id ON semantic_facts(project_id);
CREATE INDEX IF NOT EXISTS idx_security_logs_created ON security_logs(created_at);
CREATE INDEX IF NOT EXISTS idx_security_logs_type ON security_logs(event_type);
CREATE INDEX IF NOT EXISTS idx_operational_logs_created ON operational_logs(created_at);
CREATE INDEX IF NOT EXISTS idx_operational_logs_event_type ON operational_logs(event_type);
CREATE INDEX IF NOT EXISTS idx_operational_logs_tool_name ON operational_logs(tool_name);
CREATE INDEX IF NOT EXISTS idx_operational_logs_success ON operational_logs(success);
CREATE INDEX IF NOT EXISTS idx_operational_logs_policy_version ON operational_logs(policy_version);
CREATE INDEX IF NOT EXISTS idx_operational_logs_strategy_version ON operational_logs(strategy_version);
CREATE INDEX IF NOT EXISTS idx_llm_usage_created ON llm_usage(created_at);
CREATE INDEX IF NOT EXISTS idx_llm_usage_model ON llm_usage(model);
CREATE INDEX IF NOT EXISTS idx_llm_usage_provider ON llm_usage(provider);
CREATE INDEX IF NOT EXISTS idx_llm_usage_channel ON llm_usage(channel);
CREATE INDEX IF NOT EXISTS idx_user_preferences_key ON user_preferences(key);
CREATE INDEX IF NOT EXISTS idx_user_preferences_project ON user_preferences(project_id);
CREATE INDEX IF NOT EXISTS idx_user_data_kind ON user_data_items(kind);
CREATE INDEX IF NOT EXISTS idx_user_data_conversation ON user_data_items(conversation_id);
CREATE INDEX IF NOT EXISTS idx_user_data_url ON user_data_items(url);
CREATE INDEX IF NOT EXISTS idx_user_data_project ON user_data_items(project_id);
CREATE INDEX IF NOT EXISTS idx_user_data_updated ON user_data_items(updated_at);
CREATE INDEX IF NOT EXISTS idx_knowledge_project ON knowledge_items(project_id);
CREATE INDEX IF NOT EXISTS idx_knowledge_updated ON knowledge_items(updated_at);
CREATE INDEX IF NOT EXISTS idx_execution_runs_status ON execution_runs(status);
CREATE INDEX IF NOT EXISTS idx_execution_runs_stage ON execution_runs(current_stage);
CREATE INDEX IF NOT EXISTS idx_execution_runs_updated_at ON execution_runs(updated_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_execution_runs_request_id ON execution_runs(request_id) WHERE request_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_run_checkpoints_run_id ON run_checkpoints(run_id);
CREATE INDEX IF NOT EXISTS idx_tool_attempts_run_id ON tool_attempts(run_id);
"#,
    },
    Migration {
        version: 2,
        name: "experience_graph_v1",
        sql: r#"
CREATE TABLE IF NOT EXISTS experience_runs (
    id TEXT PRIMARY KEY,
    execution_run_id TEXT REFERENCES execution_runs(id) ON DELETE SET NULL,
    trace_id TEXT REFERENCES execution_traces(id) ON DELETE SET NULL,
    conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    channel TEXT NOT NULL DEFAULT '',
    scope TEXT NOT NULL DEFAULT 'global',
    intent_key TEXT NOT NULL,
    task_type TEXT,
    request_text TEXT,
    tool_sequence_digest TEXT,
    tool_sequence_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    strategy_version TEXT,
    policy_version TEXT,
    prompt_version TEXT,
    model_slot TEXT,
    success_state TEXT NOT NULL DEFAULT 'provisional',
    correction_state TEXT NOT NULL DEFAULT 'none',
    outcome_summary TEXT,
    failure_reason TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    consolidated BOOLEAN NOT NULL DEFAULT FALSE,
    accepted_at TEXT,
    corrected_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS experience_items (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    scope TEXT NOT NULL DEFAULT 'global',
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    normalized_key TEXT NOT NULL,
    confidence DOUBLE PRECISION NOT NULL DEFAULT 0.5,
    support_count INTEGER NOT NULL DEFAULT 0,
    contradiction_count INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'draft',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    last_supported_at TEXT,
    last_contradicted_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS experience_edges (
    id TEXT PRIMARY KEY,
    source_ref TEXT NOT NULL,
    source_kind TEXT NOT NULL,
    target_ref TEXT NOT NULL,
    target_kind TEXT NOT NULL,
    edge_type TEXT NOT NULL,
    weight DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    source_run_id TEXT REFERENCES experience_runs(id) ON DELETE SET NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS procedural_patterns (
    id TEXT PRIMARY KEY,
    intent_key TEXT NOT NULL,
    scope TEXT NOT NULL DEFAULT 'global',
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    trigger_summary TEXT NOT NULL DEFAULT '',
    summary TEXT NOT NULL DEFAULT '',
    tool_sequence_digest TEXT,
    steps_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    tool_sequence_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    sample_count INTEGER NOT NULL DEFAULT 0,
    success_count INTEGER NOT NULL DEFAULT 0,
    correction_count INTEGER NOT NULL DEFAULT 0,
    success_rate DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    last_validated_at TEXT,
    status TEXT NOT NULL DEFAULT 'draft',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS learning_candidates (
    id TEXT PRIMARY KEY,
    candidate_type TEXT NOT NULL,
    subject_key TEXT NOT NULL,
    title TEXT NOT NULL,
    summary TEXT,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
    pattern_id TEXT REFERENCES procedural_patterns(id) ON DELETE SET NULL,
    evidence_refs JSONB NOT NULL DEFAULT '[]'::jsonb,
    proposed_content JSONB NOT NULL DEFAULT '{}'::jsonb,
    confidence DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    approval_status TEXT NOT NULL DEFAULT 'draft',
    review_notes TEXT,
    reviewed_at TEXT,
    approved_ref TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_experience_runs_execution_run
    ON experience_runs(execution_run_id)
    WHERE execution_run_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_experience_runs_conversation_created
    ON experience_runs(conversation_id, created_at);
CREATE INDEX IF NOT EXISTS idx_experience_runs_project_created
    ON experience_runs(project_id, created_at);
CREATE INDEX IF NOT EXISTS idx_experience_runs_scope_state
    ON experience_runs(scope, success_state, correction_state, consolidated);
CREATE INDEX IF NOT EXISTS idx_experience_runs_intent
    ON experience_runs(intent_key, task_type, tool_sequence_digest);
CREATE INDEX IF NOT EXISTS idx_experience_runs_updated
    ON experience_runs(updated_at);
CREATE INDEX IF NOT EXISTS idx_experience_runs_metadata_gin
    ON experience_runs USING GIN(metadata);
CREATE INDEX IF NOT EXISTS idx_experience_runs_tools_gin
    ON experience_runs USING GIN(tool_sequence_json);

CREATE UNIQUE INDEX IF NOT EXISTS idx_experience_items_scope_key
    ON experience_items(kind, scope, COALESCE(project_id, ''), COALESCE(conversation_id, ''), normalized_key);
CREATE INDEX IF NOT EXISTS idx_experience_items_scope_status
    ON experience_items(scope, status, kind, updated_at);
CREATE INDEX IF NOT EXISTS idx_experience_items_project_status
    ON experience_items(project_id, status, updated_at);
CREATE INDEX IF NOT EXISTS idx_experience_items_conversation_status
    ON experience_items(conversation_id, status, updated_at);
CREATE INDEX IF NOT EXISTS idx_experience_items_metadata_gin
    ON experience_items USING GIN(metadata);
CREATE INDEX IF NOT EXISTS idx_experience_items_fts
    ON experience_items USING GIN (to_tsvector('simple', COALESCE(title, '') || ' ' || COALESCE(content, '')));

CREATE INDEX IF NOT EXISTS idx_experience_edges_source
    ON experience_edges(source_ref, source_kind, edge_type);
CREATE INDEX IF NOT EXISTS idx_experience_edges_target
    ON experience_edges(target_ref, target_kind, edge_type);
CREATE INDEX IF NOT EXISTS idx_experience_edges_source_run
    ON experience_edges(source_run_id);
CREATE INDEX IF NOT EXISTS idx_experience_edges_metadata_gin
    ON experience_edges USING GIN(metadata);

CREATE UNIQUE INDEX IF NOT EXISTS idx_procedural_patterns_scope_key
    ON procedural_patterns(scope, COALESCE(project_id, ''), COALESCE(conversation_id, ''), intent_key, COALESCE(tool_sequence_digest, ''));
CREATE INDEX IF NOT EXISTS idx_procedural_patterns_scope_status
    ON procedural_patterns(scope, status, updated_at);
CREATE INDEX IF NOT EXISTS idx_procedural_patterns_project_status
    ON procedural_patterns(project_id, status, updated_at);
CREATE INDEX IF NOT EXISTS idx_procedural_patterns_metadata_gin
    ON procedural_patterns USING GIN(metadata);
CREATE INDEX IF NOT EXISTS idx_procedural_patterns_steps_gin
    ON procedural_patterns USING GIN(steps_json);
CREATE INDEX IF NOT EXISTS idx_procedural_patterns_fts
    ON procedural_patterns USING GIN (to_tsvector('simple', COALESCE(title, '') || ' ' || COALESCE(trigger_summary, '') || ' ' || COALESCE(summary, '')));

CREATE INDEX IF NOT EXISTS idx_learning_candidates_status
    ON learning_candidates(approval_status, candidate_type, updated_at);
CREATE INDEX IF NOT EXISTS idx_learning_candidates_subject
    ON learning_candidates(subject_key, candidate_type);
CREATE INDEX IF NOT EXISTS idx_learning_candidates_pattern
    ON learning_candidates(pattern_id);
CREATE INDEX IF NOT EXISTS idx_learning_candidates_project
    ON learning_candidates(project_id, approval_status, updated_at);
CREATE INDEX IF NOT EXISTS idx_learning_candidates_evidence_gin
    ON learning_candidates USING GIN(evidence_refs);
CREATE INDEX IF NOT EXISTS idx_learning_candidates_content_gin
    ON learning_candidates USING GIN(proposed_content);
"#,
    },
];

pub async fn run(db: &DatabaseConnection) -> Result<()> {
    if db.get_database_backend() != DbBackend::Postgres {
        anyhow::bail!("storage migrations require a postgres database backend");
    }

    let backend = db.get_database_backend();
    db.query_one(Statement::from_string(
        backend,
        format!("SELECT pg_advisory_lock({MIGRATION_LOCK_ID})"),
    ))
    .await?;

    let result = async {
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version BIGINT PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL
            );",
        )
        .await?;

        let applied_rows = db
            .query_all(Statement::from_string(
                backend,
                "SELECT version FROM schema_migrations ORDER BY version".to_string(),
            ))
            .await?;
        let applied_versions = applied_rows
            .into_iter()
            .filter_map(|row| row.try_get::<i64>("", "version").ok())
            .collect::<std::collections::HashSet<_>>();

        for migration in MIGRATIONS {
            if applied_versions.contains(&migration.version) {
                continue;
            }
            let txn = db.begin().await?;
            txn.execute_unprepared(migration.sql).await?;
            txn.execute(statement_with_values(
                backend,
                "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?, ?, ?)"
                    .to_string(),
                vec![
                    migration.version.into(),
                    migration.name.to_string().into(),
                    chrono::Utc::now().to_rfc3339().into(),
                ],
            ))
            .await?;
            txn.commit().await?;
        }

        Ok::<(), anyhow::Error>(())
    }
    .await;

    let _ = db
        .query_one(Statement::from_string(
            backend,
            format!("SELECT pg_advisory_unlock({MIGRATION_LOCK_ID})"),
        ))
        .await;

    result
}
