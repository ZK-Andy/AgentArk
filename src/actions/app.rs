//! App deployment — write files, optionally start a server, return a live URL.
//!
//! Supports any kind of app:
//! - Static HTML/JS/CSS → served directly at /apps/{id}/
//! - Python server (FastAPI, Flask, etc.) → started as subprocess, reverse-proxied
//! - Node.js server (Express, etc.) → started as subprocess, reverse-proxied
//!
//! Dynamic apps get an auto-assigned port on localhost. The main HTTP server
//! reverse-proxies /apps/{id}/* to that port.

use anyhow::{Context, Result};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;

use crate::core::StreamEvent;

/// Port range for dynamic apps (localhost only)
const PORT_RANGE_START: u16 = 9100;
const PORT_RANGE_END: u16 = 9200;
const DEFAULT_APP_RUNTIME_IMAGE: &str = "agentark-sandbox:latest";
const APP_CONTAINER_PREFIX: &str = "agentark-app-";
const MAX_APP_COMMAND_LEN: usize = 1024;

fn default_runtime_image() -> String {
    std::env::var("AGENTARK_APP_IMAGE")
        .or_else(|_| std::env::var("APP_DEPLOY_IMAGE"))
        .unwrap_or_else(|_| DEFAULT_APP_RUNTIME_IMAGE.to_string())
}

fn app_container_name(app_id: &str) -> String {
    format!("{}{}", APP_CONTAINER_PREFIX, app_id)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppRequiredInput {
    pub key: String,
    #[serde(default = "default_required_input_sensitive")]
    pub sensitive: bool,
}

fn default_required_input_sensitive() -> bool {
    true
}

fn push_required_input(out: &mut Vec<AppRequiredInput>, key: &str, sensitive: bool) {
    let k = key.trim();
    if k.is_empty() {
        return;
    }
    if let Some(existing) = out.iter_mut().find(|r| r.key == k) {
        // If any declaration marks it sensitive, keep it sensitive.
        existing.sensitive = existing.sensitive || sensitive;
        return;
    }
    out.push(AppRequiredInput {
        key: k.to_string(),
        sensitive,
    });
}

fn collect_required_string_list(
    out: &mut Vec<AppRequiredInput>,
    arr: Option<&Vec<serde_json::Value>>,
    sensitive: bool,
) {
    let Some(arr) = arr else {
        return;
    };
    for item in arr {
        if let Some(key) = item.as_str() {
            push_required_input(out, key, sensitive);
        }
    }
}

pub fn parse_required_inputs(arguments: &serde_json::Value) -> Vec<AppRequiredInput> {
    let mut out = Vec::new();
    // New generic model.
    if let Some(arr) = arguments.get("required_inputs").and_then(|v| v.as_array()) {
        for item in arr {
            match item {
                serde_json::Value::String(key) => push_required_input(&mut out, key, true),
                serde_json::Value::Object(obj) => {
                    let key = obj
                        .get("key")
                        .and_then(|v| v.as_str())
                        .or_else(|| obj.get("name").and_then(|v| v.as_str()))
                        .or_else(|| obj.get("env").and_then(|v| v.as_str()))
                        .unwrap_or("");
                    let sensitive = obj
                        .get("sensitive")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    push_required_input(&mut out, key, sensitive);
                }
                _ => {}
            }
        }
    }

    // Compatibility aliases.
    collect_required_string_list(
        &mut out,
        arguments.get("required_secrets").and_then(|v| v.as_array()),
        true,
    );
    collect_required_string_list(
        &mut out,
        arguments.get("required_env").and_then(|v| v.as_array()),
        true,
    );
    collect_required_string_list(
        &mut out,
        arguments.get("required_config").and_then(|v| v.as_array()),
        false,
    );
    out
}

pub fn parse_config_values(arguments: &serde_json::Value) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Some(obj) = arguments.get("config").and_then(|v| v.as_object()) else {
        return out;
    };
    for (k, v) in obj {
        let value = match v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => continue,
        };
        if !value.trim().is_empty() {
            out.insert(k.clone(), value);
        }
    }
    out
}

fn resolve_secret_value(
    custom: &std::collections::HashMap<String, String>,
    llm_env: &HashMap<String, String>,
    env: &str,
) -> Option<String> {
    if let Some(v) = custom
        .get(&format!("env:{}", env))
        .or_else(|| custom.get(&format!("secret:{}", env)))
        .or_else(|| custom.get(env))
    {
        if !v.trim().is_empty() {
            return Some(v.clone());
        }
    }

    for key in crate::core::secrets::storage_keys_for_user_key(env) {
        if let Some(v) = custom.get(&key) {
            if !v.trim().is_empty() {
                return Some(v.clone());
            }
        }
    }

    let allow_llm_env_passthrough = std::env::var("AGENTARK_ALLOW_LLM_ENV_TO_APPS")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false);
    if allow_llm_env_passthrough {
        llm_env.get(env).cloned()
    } else {
        None
    }
}

pub async fn resolve_required_env_values(
    config_dir: &Path,
    data_dir: &Path,
    required_inputs: &[AppRequiredInput],
    llm_env: &HashMap<String, String>,
    config_values: &HashMap<String, String>,
) -> Result<(HashMap<String, String>, Vec<String>, Vec<String>)> {
    let mgr =
        crate::core::config::SecureConfigManager::new_with_data_dir(config_dir, Some(data_dir))?;
    let secrets = mgr.load_secrets().unwrap_or_default();
    let mut resolved = HashMap::new();
    let mut missing_sensitive = Vec::new();
    let mut missing_config = Vec::new();

    for required in required_inputs {
        let key = required.key.trim();
        if key.is_empty() {
            continue;
        }
        if required.sensitive {
            if let Some(v) = resolve_secret_value(&secrets.custom, llm_env, key) {
                resolved.insert(key.to_string(), v);
            } else if !missing_sensitive.iter().any(|m| m == key) {
                missing_sensitive.push(key.to_string());
            }
            continue;
        }

        if let Some(v) = config_values.get(key).cloned() {
            resolved.insert(key.to_string(), v);
            continue;
        }

        // Fallback: allow resolving non-sensitive values from encrypted store if user chose to save there.
        if let Some(v) = resolve_secret_value(&secrets.custom, llm_env, key) {
            resolved.insert(key.to_string(), v);
        } else if !missing_config.iter().any(|m| m == key) {
            missing_config.push(key.to_string());
        }
    }
    Ok((resolved, missing_sensitive, missing_config))
}

fn normalize_mount_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn validate_app_command(command: &str, label: &str) -> Result<()> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        anyhow::bail!("{} cannot be empty", label);
    }
    if trimmed.len() > MAX_APP_COMMAND_LEN {
        anyhow::bail!(
            "{} is too long ({} chars, max {})",
            label,
            trimmed.len(),
            MAX_APP_COMMAND_LEN
        );
    }
    if trimmed.contains('\n') || trimmed.contains('\r') {
        anyhow::bail!("{} must be a single-line command", label);
    }

    // Reject shell chaining/injection operators. Dynamic apps can still run arbitrary binaries,
    // but only as a single command with arguments.
    let blocked_tokens = ["&&", "||", ";", "|", "`", "$(", "<", ">"];
    if blocked_tokens.iter().any(|tok| trimmed.contains(tok)) {
        anyhow::bail!(
            "{} contains blocked shell control operators; provide a single executable command with args",
            label
        );
    }
    Ok(())
}

fn is_valid_env_key(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 128
        && key
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

async fn write_runtime_env_file(
    app_dir: &Path,
    extra_env: &HashMap<String, String>,
) -> Result<Option<PathBuf>> {
    if extra_env.is_empty() {
        return Ok(None);
    }

    let mut ordered: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in extra_env {
        if !is_valid_env_key(k) {
            anyhow::bail!("Invalid env key '{}': use [A-Z0-9_]", k);
        }
        if v.contains('\0') || v.contains('\n') || v.contains('\r') {
            anyhow::bail!("Env value for '{}' contains unsupported control characters", k);
        }
        ordered.insert(k.clone(), v.clone());
    }

    let env_file_path = app_dir.join(".agentark_runtime_env");
    let mut content = String::new();
    for (k, v) in ordered {
        content.push_str(&k);
        content.push('=');
        content.push_str(&v);
        content.push('\n');
    }
    tokio::fs::write(&env_file_path, content)
        .await
        .with_context(|| format!("failed to write runtime env file at {:?}", env_file_path))?;

    Ok(Some(env_file_path))
}

async fn run_docker(
    args: &[String],
    cwd: Option<&Path>,
    timeout_secs: u64,
) -> Result<std::process::Output> {
    let mut cmd = tokio::process::Command::new("docker");
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let fut = cmd.output();
    tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), fut)
        .await
        .map_err(|_| anyhow::anyhow!("docker command timed out"))?
        .map_err(|e| anyhow::anyhow!("failed to execute docker: {}", e))
}

async fn cleanup_existing_container(name: &str) {
    let args = vec!["rm".to_string(), "-f".to_string(), name.to_string()];
    let _ = run_docker(&args, None, 20).await;
}

async fn is_container_running(container_id: &str) -> bool {
    let args = vec![
        "inspect".to_string(),
        "-f".to_string(),
        "{{.State.Running}}".to_string(),
        container_id.to_string(),
    ];
    match run_docker(&args, None, 15).await {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .trim()
            .eq_ignore_ascii_case("true"),
        _ => false,
    }
}

async fn stop_container(container_id: &str) -> Result<()> {
    let stop_args = vec![
        "stop".to_string(),
        "-t".to_string(),
        "10".to_string(),
        container_id.to_string(),
    ];
    let output = run_docker(&stop_args, None, 30).await?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("No such container") {
        return Ok(());
    }
    anyhow::bail!(
        "failed to stop container {}: {}",
        container_id,
        stderr.trim()
    );
}

async fn stop_child_process(child: &mut tokio::process::Child, app_id: &str) -> Result<()> {
    let already_exited = matches!(child.try_wait(), Ok(Some(_)));
    if already_exited {
        return Ok(());
    }
    child
        .kill()
        .await
        .with_context(|| format!("failed to kill app process {}", app_id))?;
    tokio::time::timeout(std::time::Duration::from_secs(5), child.wait())
        .await
        .map_err(|_| anyhow::anyhow!("timeout waiting for process {} to exit", app_id))?
        .with_context(|| format!("failed waiting for app process {}", app_id))?;
    Ok(())
}

pub async fn launch_dynamic_container(
    app_id: &str,
    app_dir: &Path,
    entry_command: &str,
    install_command: Option<&str>,
    port: u16,
    extra_env: &HashMap<String, String>,
    runtime_image: Option<&str>,
) -> Result<String> {
    let container_name = app_container_name(app_id);
    cleanup_existing_container(&container_name).await;

    validate_app_command(entry_command, "entry_command")?;
    if let Some(cmd) = install_command {
        validate_app_command(cmd, "install_command")?;
    }

    let mut script_parts: Vec<String> = Vec::new();
    script_parts.push("set -e".to_string());
    script_parts.push("export PATH=\"/workspace/node_modules/.bin:$PATH\"".to_string());
    script_parts
        .push("export PYTHONPATH=\"/workspace/_deps${PYTHONPATH:+:$PYTHONPATH}\"".to_string());
    if let Some(cmd) = install_command {
        let trimmed = cmd.trim();
        if !trimmed.is_empty() {
            script_parts.push(trimmed.to_string());
        }
    }
    script_parts.push(entry_command.trim().to_string());
    let launch_script = script_parts
        .join(" && ")
        .replace("{PORT}", &port.to_string());

    let image = runtime_image
        .map(|s| s.to_string())
        .unwrap_or_else(default_runtime_image);
    let mount = normalize_mount_path(app_dir);
    let mut args = vec![
        "run".to_string(),
        "-d".to_string(),
        "--rm".to_string(),
        "--name".to_string(),
        container_name,
        "-p".to_string(),
        format!("127.0.0.1:{0}:{0}", port),
        "-v".to_string(),
        format!("{}:/workspace", mount),
        "-w".to_string(),
        "/workspace".to_string(),
        "-e".to_string(),
        format!("PORT={}", port),
        "-e".to_string(),
        "HOST=0.0.0.0".to_string(),
    ];
    let env_file_path = write_runtime_env_file(app_dir, extra_env).await?;
    if let Some(path) = env_file_path.as_ref() {
        args.push("--env-file".to_string());
        args.push(path.to_string_lossy().to_string());
    }
    args.push(image);
    args.push("sh".to_string());
    args.push("-lc".to_string());
    args.push(launch_script);

    let output = run_docker(&args, None, 90).await;
    if let Some(path) = env_file_path {
        let _ = tokio::fs::remove_file(path).await;
    }
    let output = output?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("docker run failed: {}", stderr.trim());
    }
    let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if container_id.is_empty() {
        anyhow::bail!("docker run did not return a container id");
    }
    Ok(container_id)
}

fn emit_progress(stream_tx: &Option<Sender<StreamEvent>>, message: &str) {
    if let Some(tx) = stream_tx {
        let _ = tx.try_send(StreamEvent::ToolResult {
            name: "app_deploy".to_string(),
            content: message.to_string(),
        });
    }
}

/// A running app process
pub struct RunningApp {
    pub title: String,
    pub port: Option<u16>,
    pub process: Option<tokio::process::Child>,
    pub container_id: Option<String>,
    pub app_dir: PathBuf,
    pub is_static: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_accessed: chrono::DateTime<chrono::Utc>,
    /// Rolling request count since last pulse check (for traffic monitoring)
    pub request_count: u64,
    /// Random access key for app authentication
    pub access_key: String,
}

/// Generate a random access key for app authentication
pub fn generate_access_key() -> String {
    format!("ak_{}", uuid::Uuid::new_v4().simple())
}

/// Snapshot of an app's health for ArkPulse reporting
pub struct AppHealthSnapshot {
    pub id: String,
    pub title: String,
    pub is_static: bool,
    pub process_alive: bool,
    pub requests_since_last_check: u64,
    pub last_accessed: chrono::DateTime<chrono::Utc>,
}

/// Global app registry — tracks deployed apps and their processes
#[derive(Clone)]
pub struct AppRegistry {
    apps: Arc<RwLock<HashMap<String, Arc<RwLock<RunningApp>>>>>,
}

impl AppRegistry {
    pub fn new() -> Self {
        Self {
            apps: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// List all deployed apps
    pub async fn list(&self) -> Vec<serde_json::Value> {
        let app_entries: Vec<(String, Arc<RwLock<RunningApp>>)> = {
            let apps = self.apps.read().await;
            apps.iter()
                .map(|(id, app)| (id.clone(), Arc::clone(app)))
                .collect()
        };
        let mut result = Vec::new();
        for (id, app) in app_entries {
            let mut app = app.write().await;
            let mut mark_stopped = false;
            let running = if app.is_static {
                true
            } else if let Some(container_id) = app.container_id.as_ref() {
                let up = is_container_running(container_id).await;
                if !up {
                    mark_stopped = true;
                }
                up
            } else if let Some(child) = app.process.as_mut() {
                match child.try_wait() {
                    Ok(None) => true,
                    Ok(Some(_)) => {
                        mark_stopped = true;
                        false
                    }
                    Err(_) => false,
                }
            } else {
                false
            };
            if mark_stopped {
                app.process = None;
                app.container_id = None;
                app.port = None;
            }
            result.push(serde_json::json!({
                "id": id,
                "title": app.title,
                "port": app.port,
                "is_static": app.is_static,
                "running": running,
                "created_at": app.created_at.to_rfc3339(),
                "url": format!("/apps/{}/", id),
                "access_url": format!("/apps/{}/?key={}", id, app.access_key),
            }));
        }
        result
    }

    /// Get the port for a dynamic app (for reverse proxy)
    pub async fn get_port(&self, app_id: &str) -> Option<u16> {
        let app_handle = {
            let apps = self.apps.read().await;
            apps.get(app_id).cloned()
        }?;
        let app = app_handle.read().await;
        app.port
    }

    /// Get the app directory path
    pub async fn get_dir(&self, app_id: &str) -> Option<PathBuf> {
        let app_handle = {
            let apps = self.apps.read().await;
            apps.get(app_id).cloned()
        }?;
        let app = app_handle.read().await;
        Some(app.app_dir.clone())
    }

    /// Check if app is static
    pub async fn is_static(&self, app_id: &str) -> bool {
        let app_handle = {
            let apps = self.apps.read().await;
            apps.get(app_id).cloned()
        };
        if let Some(app) = app_handle {
            return app.read().await.is_static;
        }
        false
    }

    /// Register a static app
    pub async fn register_static(
        &self,
        id: String,
        title: String,
        app_dir: PathBuf,
        access_key: String,
    ) {
        let app = RunningApp {
            title,
            port: None,
            process: None,
            container_id: None,
            app_dir,
            is_static: true,
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            request_count: 0,
            access_key,
        };
        self.apps
            .write()
            .await
            .insert(id, Arc::new(RwLock::new(app)));
    }

    /// Register and start a dynamic app
    pub async fn register_dynamic(
        &self,
        id: String,
        title: String,
        app_dir: PathBuf,
        child: Option<tokio::process::Child>,
        container_id: Option<String>,
        port: u16,
        access_key: String,
    ) {
        let app = RunningApp {
            title,
            port: Some(port),
            process: child,
            container_id,
            app_dir,
            is_static: false,
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            request_count: 0,
            access_key,
        };
        self.apps
            .write()
            .await
            .insert(id, Arc::new(RwLock::new(app)));
    }

    /// Verify access key for an app
    pub async fn verify_key(&self, app_id: &str, key: &str) -> bool {
        let app_handle = {
            let apps = self.apps.read().await;
            apps.get(app_id).cloned()
        };
        if let Some(app) = app_handle {
            let app = app.read().await;
            return app.access_key == key;
        }
        false
    }

    /// Record an access (called when an app is served via HTTP)
    pub async fn touch(&self, app_id: &str) {
        let app_handle = {
            let apps = self.apps.read().await;
            apps.get(app_id).cloned()
        };
        if let Some(app) = app_handle {
            let mut app = app.write().await;
            app.last_accessed = chrono::Utc::now();
            app.request_count += 1;
        }
    }

    /// Get a health snapshot of all apps for ArkPulse, resetting request counters
    pub async fn pulse_snapshot(&self) -> Vec<AppHealthSnapshot> {
        let app_entries: Vec<(String, Arc<RwLock<RunningApp>>)> = {
            let apps = self.apps.read().await;
            apps.iter()
                .map(|(id, app)| (id.clone(), Arc::clone(app)))
                .collect()
        };
        let mut snapshots = Vec::new();
        for (id, app) in app_entries {
            let mut app = app.write().await;
            let mut mark_stopped = false;
            let process_alive = if app.is_static {
                true
            } else if let Some(container_id) = app.container_id.as_ref() {
                let up = is_container_running(container_id).await;
                if !up {
                    mark_stopped = true;
                }
                up
            } else if let Some(child) = app.process.as_mut() {
                match child.try_wait() {
                    Ok(None) => true,
                    Ok(Some(_)) => {
                        mark_stopped = true;
                        false
                    }
                    Err(_) => false,
                }
            } else {
                false
            };
            if mark_stopped {
                app.process = None;
                app.container_id = None;
                app.port = None;
            }
            snapshots.push(AppHealthSnapshot {
                id,
                title: app.title.clone(),
                is_static: app.is_static,
                process_alive,
                requests_since_last_check: app.request_count,
                last_accessed: app.last_accessed,
            });
            app.request_count = 0; // Reset counter after snapshot
        }
        snapshots
    }

    /// Get apps that haven't been accessed in the given duration
    pub async fn get_unused_apps(
        &self,
        idle_hours: i64,
    ) -> Vec<(String, String, chrono::DateTime<chrono::Utc>)> {
        let cutoff = chrono::Utc::now() - chrono::Duration::hours(idle_hours);
        let app_entries: Vec<(String, Arc<RwLock<RunningApp>>)> = {
            let apps = self.apps.read().await;
            apps.iter()
                .map(|(id, app)| (id.clone(), Arc::clone(app)))
                .collect()
        };
        let mut unused = Vec::new();
        for (id, app) in app_entries {
            let app = app.read().await;
            if app.last_accessed < cutoff {
                unused.push((id, app.title.clone(), app.last_accessed));
            }
        }
        unused
    }

    /// Stop runtime process for a dynamic app but keep app metadata registered.
    pub async fn stop_runtime(&self, app_id: &str) -> Result<()> {
        let app_handle = {
            let apps = self.apps.read().await;
            apps.get(app_id).cloned()
        };
        let Some(app) = app_handle else {
            return Ok(());
        };
        let mut app = app.write().await;
        if app.is_static {
            return Ok(());
        }
        let mut child = app.process.take();
        let container_id = app.container_id.take();
        app.port = None;
        drop(app);

        if let Some(ref cid) = container_id {
            stop_container(cid).await?;
            tracing::info!("Stopped app container: {} ({})", app_id, cid);
        }
        if let Some(ref mut c) = child {
            stop_child_process(c, app_id).await?;
            tracing::info!("Stopped app process: {}", app_id);
        }
        Ok(())
    }

    /// Stop and remove an app
    pub async fn stop(&self, app_id: &str) -> Result<()> {
        let app_handle = {
            let apps = self.apps.read().await;
            apps.get(app_id).cloned()
        };
        if let Some(app) = app_handle {
            let mut app = app.write().await;
            let mut child = app.process.take();
            let container_id = app.container_id.take();
            app.port = None;
            drop(app);

            if let Some(ref cid) = container_id {
                stop_container(cid).await?;
                tracing::info!("Stopped app container: {} ({})", app_id, cid);
            }
            if let Some(ref mut c) = child {
                stop_child_process(c, app_id).await?;
                tracing::info!("Stopped app process: {}", app_id);
            }
            self.apps.write().await.remove(app_id);
        }
        Ok(())
    }

    /// Find an available port in the range
    pub async fn find_available_port(&self) -> Option<u16> {
        let apps = self.apps.read().await;
        let used_ports: Vec<u16> = apps
            .values()
            .filter_map(|a| {
                // We can't await inside filter_map in a sync context, so use try_read
                if let Ok(app) = a.try_read() {
                    app.port
                } else {
                    None
                }
            })
            .collect();

        for port in PORT_RANGE_START..PORT_RANGE_END {
            if !used_ports.contains(&port) {
                // Quick check if port is actually free
                if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
                    return Some(port);
                }
            }
        }
        None
    }

    /// Restore apps from disk on startup. Static apps are served immediately.
    /// Dynamic apps with entry_command are restarted automatically.
    pub async fn restore_from_disk(
        &self,
        config_dir: &Path,
        data_dir: &Path,
        llm_env: &HashMap<String, String>,
    ) {
        let apps_dir = data_dir.join("apps");
        if !apps_dir.exists() {
            return;
        }
        if let Ok(mut entries) = tokio::fs::read_dir(&apps_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_dir() {
                    let id = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    if id.is_empty() {
                        continue;
                    }

                    // Read metadata
                    let meta_path = path.join(".app_meta.json");
                    let meta: Option<serde_json::Value> = tokio::fs::read(&meta_path)
                        .await
                        .ok()
                        .and_then(|bytes| serde_json::from_slice(&bytes).ok());

                    let title = meta
                        .as_ref()
                        .and_then(|m| m.get("title").and_then(|t| t.as_str()))
                        .unwrap_or(&id)
                        .to_string();

                    let entry_command = meta
                        .as_ref()
                        .and_then(|m| m.get("entry_command").and_then(|c| c.as_str()))
                        .map(|s| s.to_string());
                    let install_command = meta
                        .as_ref()
                        .and_then(|m| m.get("install_command").and_then(|c| c.as_str()))
                        .map(|s| s.to_string());
                    let runtime_image = meta
                        .as_ref()
                        .and_then(|m| m.get("runtime_image").and_then(|c| c.as_str()))
                        .map(|s| s.to_string());
                    let required_inputs =
                        meta.as_ref().map(parse_required_inputs).unwrap_or_default();
                    let config_values: HashMap<String, String> = meta
                        .as_ref()
                        .and_then(|m| m.get("config_values").and_then(|v| v.as_object()))
                        .map(|obj| {
                            obj.iter()
                                .filter_map(|(k, v)| {
                                    let value = match v {
                                        serde_json::Value::String(s) => s.clone(),
                                        serde_json::Value::Bool(b) => b.to_string(),
                                        serde_json::Value::Number(n) => n.to_string(),
                                        _ => return None,
                                    };
                                    Some((k.clone(), value))
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    // Restore or regenerate access key
                    let access_key = meta
                        .as_ref()
                        .and_then(|m| m.get("access_key").and_then(|k| k.as_str()))
                        .map(|s| s.to_string())
                        .unwrap_or_else(generate_access_key);

                    if let Some(entry_cmd) = entry_command {
                        // Dynamic app — restart in isolated container runtime
                        if let Some(port) = self.find_available_port().await {
                            tracing::info!(
                                "Restarting app '{}' (id={}) on port {}",
                                title,
                                id,
                                port
                            );
                            let (resolved_env, missing_sensitive, missing_config) =
                                match resolve_required_env_values(
                                    config_dir,
                                    data_dir,
                                    &required_inputs,
                                    llm_env,
                                    &config_values,
                                )
                                .await
                                {
                                    Ok(out) => out,
                                    Err(e) => {
                                        tracing::warn!(
                                        "Failed to resolve secrets for app {} during restore: {}",
                                        id,
                                        e
                                    );
                                        self.register_static(
                                            id.clone(),
                                            title,
                                            path,
                                            access_key.clone(),
                                        )
                                        .await;
                                        continue;
                                    }
                                };
                            if !missing_sensitive.is_empty() || !missing_config.is_empty() {
                                tracing::warn!(
                                    "Skipping dynamic restore for app '{}' (id={}): missing_sensitive={:?}, missing_config={:?}",
                                    title,
                                    id,
                                    missing_sensitive,
                                    missing_config
                                );
                                self.register_static(id.clone(), title, path, access_key.clone())
                                    .await;
                                continue;
                            }
                            match launch_dynamic_container(
                                &id,
                                &path,
                                &entry_cmd,
                                install_command.as_deref(),
                                port,
                                &resolved_env,
                                runtime_image.as_deref(),
                            )
                            .await
                            {
                                Ok(container_id) => {
                                    self.register_dynamic(
                                        id.clone(),
                                        title,
                                        path,
                                        None,
                                        Some(container_id),
                                        port,
                                        access_key.clone(),
                                    )
                                    .await;
                                    tracing::info!("Restarted dynamic app: {}", id);
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to restart app {}: {}", id, e);
                                    // Register as static fallback so files are still accessible.
                                    self.register_static(
                                        id.clone(),
                                        title,
                                        path,
                                        access_key.clone(),
                                    )
                                    .await;
                                }
                            }
                        } else {
                            tracing::warn!("No available port to restart app {}", id);
                            self.register_static(id.clone(), title, path, access_key.clone())
                                .await;
                        }
                    } else {
                        // Static app
                        self.register_static(id.clone(), title, path, access_key.clone())
                            .await;
                        tracing::info!("Restored static app: {}", id);
                    }
                }
            }
        }
    }
}

/// Deploy an app from agent-generated files.
///
/// Arguments (JSON):
/// - `files`: object mapping filename → content (required)
/// - `title`: app name (optional, default: "App")
/// - `entry_command`: command to start the server (optional — if omitted, static)
/// - `port`: port the server listens on (optional — auto-assigned if dynamic)
/// - `install_command`: command to install deps (optional, e.g. "pip install -r requirements.txt")
///
/// Returns JSON with the app URL.
pub async fn app_deploy(
    config_dir: &Path,
    data_dir: &Path,
    arguments: &serde_json::Value,
    registry: &AppRegistry,
    llm_env: &HashMap<String, String>,
    stream_tx: Option<Sender<StreamEvent>>,
) -> Result<String> {
    let files = arguments
        .get("files")
        .and_then(|v| v.as_object())
        .ok_or_else(|| {
            anyhow::anyhow!("Missing 'files' — provide an object mapping filename to content")
        })?;

    if files.is_empty() {
        anyhow::bail!("'files' must contain at least one file");
    }
    let file_count = files.len();

    let title = arguments
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("App");
    let entry_command = arguments.get("entry_command").and_then(|v| v.as_str());
    let install_command = arguments.get("install_command").and_then(|v| v.as_str());
    let runtime_image = arguments
        .get("runtime_image")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let required_inputs = parse_required_inputs(arguments);
    let config_values = parse_config_values(arguments);
    let is_static = entry_command.is_none();

    // Generate app ID and access key
    let app_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let access_key = generate_access_key();
    let app_dir = data_dir.join("apps").join(&app_id);
    tokio::fs::create_dir_all(&app_dir).await?;

    tracing::info!(
        "Deploying app '{}' (id={}, static={})",
        title,
        app_id,
        is_static
    );
    emit_progress(
        &stream_tx,
        &format!(
            "Deploying '{}' ({})",
            title,
            if is_static { "static" } else { "dynamic" }
        ),
    );
    emit_progress(&stream_tx, "Writing files...");

    // Write all files
    let mut written_files = 0usize;
    let mut skipped_files = 0usize;
    for (filename, content) in files {
        let content_str = content.as_str().unwrap_or_default();
        // Prevent path traversal
        if filename.contains("..") || filename.starts_with('/') || filename.starts_with('\\') {
            tracing::warn!("Skipping file with suspicious path: {}", filename);
            skipped_files += 1;
            continue;
        }
        let file_path = app_dir.join(filename);
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&file_path, content_str)
            .await
            .with_context(|| format!("Failed to write {}", filename))?;
        written_files += 1;
    }
    if written_files == 0 {
        anyhow::bail!("No valid files were written. Check filenames and try again.");
    }
    emit_progress(
        &stream_tx,
        &format!(
            "Files written: {} (skipped: {} out of {})",
            written_files, skipped_files, file_count
        ),
    );

    let (resolved_env, missing_sensitive, missing_config) = resolve_required_env_values(
        config_dir,
        data_dir,
        &required_inputs,
        llm_env,
        &config_values,
    )
    .await?;

    let required_secret_keys: Vec<String> = required_inputs
        .iter()
        .filter(|r| r.sensitive)
        .map(|r| r.key.clone())
        .collect();
    let required_config_keys: Vec<String> = required_inputs
        .iter()
        .filter(|r| !r.sensitive)
        .map(|r| r.key.clone())
        .collect();

    // Save metadata for restore on restart
    let meta = serde_json::json!({
        "title": title,
        "entry_command": entry_command,
        "install_command": install_command,
        "runtime_image": runtime_image.clone(),
        "required_inputs": required_inputs.clone(),
        "required_secrets": required_secret_keys.clone(),
        "required_env": required_secret_keys.clone(),
        "required_config": required_config_keys.clone(),
        "config_values": config_values.clone(),
        "access_key": access_key,
        "created_at": chrono::Utc::now().to_rfc3339(),
    });
    tokio::fs::write(
        app_dir.join(".app_meta.json"),
        serde_json::to_string_pretty(&meta)?,
    )
    .await?;
    emit_progress(&stream_tx, "Saved app metadata");

    if is_static {
        // Static app — just register, served directly by HTTP server
        registry
            .register_static(
                app_id.clone(),
                title.to_string(),
                app_dir,
                access_key.clone(),
            )
            .await;
        let url = format!("/apps/{}/", app_id);
        tracing::info!("Static app deployed at {}", url);
        emit_progress(&stream_tx, &format!("Static app ready at {}", url));
        return Ok(serde_json::json!({
            "status": "deployed",
            "type": "static",
            "app_id": app_id,
            "url": url,
            "title": title,
            "access_key": access_key,
        })
        .to_string());
    }

    // Dynamic app — start server in isolated container runtime
    let port = arguments
        .get("port")
        .and_then(|v| v.as_u64())
        .map(|p| p as u16);

    let port = match port {
        Some(p) => p,
        None => registry.find_available_port().await.ok_or_else(|| {
            anyhow::anyhow!(
                "No available ports in range {}-{}",
                PORT_RANGE_START,
                PORT_RANGE_END
            )
        })?,
    };
    emit_progress(&stream_tx, &format!("Assigned port {}", port));

    if !missing_sensitive.is_empty() || !missing_config.is_empty() {
        let mut missing_all = missing_sensitive.clone();
        for m in &missing_config {
            if !missing_all.iter().any(|x| x == m) {
                missing_all.push(m.clone());
            }
        }
        registry
            .register_static(
                app_id.clone(),
                title.to_string(),
                app_dir,
                access_key.clone(),
            )
            .await;
        emit_progress(
            &stream_tx,
            &format!(
                "App created but waiting for required inputs: {}",
                missing_all.join(", ")
            ),
        );
        return Ok(serde_json::json!({
            "status": "needs_secrets",
            "type": "dynamic",
            "app_id": app_id,
            "title": title,
            "url": format!("/apps/{}/", app_id),
            "access_key": access_key,
            "required_inputs": required_inputs,
            "required_secrets": required_secret_keys.clone(),
            "required_env": required_secret_keys,
            "required_config": required_config_keys,
            "missing_env": missing_sensitive,
            "missing_config": missing_config,
            "message": "Missing required inputs. For sensitive keys use: set secret KEY=VALUE. For non-sensitive values pass config.{KEY} when deploying/restarting."
        })
        .to_string());
    }

    let has_requirements = app_dir.join("requirements.txt").exists();
    let has_package_json = app_dir.join("package.json").exists();

    let effective_install_cmd = if let Some(cmd) = install_command {
        Some(cmd.to_string())
    } else if has_requirements {
        Some("pip install --target /workspace/_deps -r requirements.txt -q".to_string())
    } else if has_package_json {
        Some("npm install --omit=dev".to_string())
    } else {
        None
    };

    if effective_install_cmd.is_some() {
        emit_progress(&stream_tx, "Installing dependencies...");
    } else {
        emit_progress(&stream_tx, "No dependencies to install");
    }

    // Start the server process in isolated container
    let entry = entry_command.unwrap();
    tracing::info!(
        "Starting app {} on port {} in isolated runtime",
        app_id,
        port
    );
    emit_progress(&stream_tx, &format!("Starting server on port {}", port));

    let container_id = launch_dynamic_container(
        &app_id,
        &app_dir,
        entry,
        effective_install_cmd.as_deref(),
        port,
        &resolved_env,
        runtime_image.as_deref(),
    )
    .await?;
    emit_progress(&stream_tx, "Server container started");

    registry
        .register_dynamic(
            app_id.clone(),
            title.to_string(),
            app_dir,
            None,
            Some(container_id),
            port,
            access_key.clone(),
        )
        .await;

    // Wait briefly for the server to start
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let url = format!("/apps/{}/", app_id);
    tracing::info!("Dynamic app deployed at {} (port {})", url, port);
    emit_progress(&stream_tx, &format!("Dynamic app ready at {}", url));

    Ok(serde_json::json!({
        "status": "deployed",
        "type": "dynamic",
        "app_id": app_id,
        "url": url,
        "port": port,
        "title": title,
        "access_key": access_key,
    })
    .to_string())
}
