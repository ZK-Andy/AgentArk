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
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;

use crate::core::StreamEvent;

/// Port range for dynamic apps (localhost only)
const PORT_RANGE_START: u16 = 9100;
const PORT_RANGE_END: u16 = 9200;
const DEFAULT_APP_RUNTIME_IMAGE: &str = "agentark-sandbox:latest";
const APP_CONTAINER_PREFIX: &str = "agentark-app-";
const MAX_APP_COMMAND_LEN: usize = 1024;
const LOCAL_RUNTIME_STDOUT_LOG_FILE: &str = ".agentark_runtime_stdout.log";
const LOCAL_RUNTIME_STDERR_LOG_FILE: &str = ".agentark_runtime_stderr.log";
const LOCAL_RUNTIME_LOG_TAIL_BYTES: usize = 4096;
const MAX_REPO_CLONE_TIMEOUT_SECS: u64 = 240;
const MAX_REPO_COMMAND_COUNT: usize = 120;
const MAX_REPO_TEXT_FILE_BYTES: usize = 512 * 1024;
const MAX_REPO_TOTAL_TEXT_BYTES: usize = 8 * 1024 * 1024;
const MAX_REPO_TEXT_FILES: usize = 600;
const MAX_README_BYTES: usize = 256 * 1024;

fn default_runtime_image() -> String {
    std::env::var("AGENTARK_APP_IMAGE")
        .or_else(|_| std::env::var("APP_DEPLOY_IMAGE"))
        .unwrap_or_else(|_| DEFAULT_APP_RUNTIME_IMAGE.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RepoServiceMode {
    Auto,
    Frontend,
    Backend,
    Fullstack,
}

fn repo_service_mode_from_opt(raw: Option<&str>) -> RepoServiceMode {
    match raw.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "frontend" | "front-end" | "ui" | "web" => RepoServiceMode::Frontend,
        "backend" | "back-end" | "api" | "server" => RepoServiceMode::Backend,
        "fullstack" | "full-stack" | "all" => RepoServiceMode::Fullstack,
        _ => RepoServiceMode::Auto,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RepoServiceKind {
    Frontend,
    Backend,
    Fullstack,
    Static,
}

impl RepoServiceKind {
    fn as_str(self) -> &'static str {
        match self {
            RepoServiceKind::Frontend => "frontend",
            RepoServiceKind::Backend => "backend",
            RepoServiceKind::Fullstack => "fullstack",
            RepoServiceKind::Static => "static",
        }
    }

    fn matches_mode(self, mode: RepoServiceMode) -> bool {
        match mode {
            RepoServiceMode::Auto | RepoServiceMode::Fullstack => true,
            RepoServiceMode::Frontend => {
                matches!(
                    self,
                    RepoServiceKind::Frontend
                        | RepoServiceKind::Fullstack
                        | RepoServiceKind::Static
                )
            }
            RepoServiceMode::Backend => {
                matches!(self, RepoServiceKind::Backend | RepoServiceKind::Fullstack)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RepoCopyScope {
    RepositoryRoot,
    ServiceRoot,
}

#[derive(Debug, Clone, Default)]
struct RepoReadmeHints {
    install_command: Option<String>,
    start_command: Option<String>,
    mentions_compose: bool,
}

#[derive(Debug, Clone, Default)]
struct RepoNodeManifest {
    name: Option<String>,
    scripts: HashSet<String>,
    dependencies: HashSet<String>,
    has_workspaces: bool,
}

#[derive(Debug, Clone)]
struct RepoServicePlan {
    title: String,
    relative_dir: String,
    kind: RepoServiceKind,
    copy_scope: RepoCopyScope,
    install_command: Option<String>,
    entry_command: Option<String>,
    required_inputs: Vec<AppRequiredInput>,
    detection_reason: String,
}

fn normalize_repo_relative_path(path: &Path) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    let trimmed = raw.trim_matches('/');
    trimmed
        .trim_start_matches("./")
        .trim_end_matches('/')
        .to_string()
}

fn humanize_repo_label(raw: &str) -> String {
    let mut parts = Vec::new();
    for token in raw
        .split(|ch: char| !(ch.is_ascii_alphanumeric()))
        .filter(|token| !token.is_empty())
    {
        let mut chars = token.chars();
        let Some(first) = chars.next() else {
            continue;
        };
        let mut rebuilt = String::new();
        rebuilt.push(first.to_ascii_uppercase());
        rebuilt.push_str(&chars.as_str().to_ascii_lowercase());
        parts.push(rebuilt);
    }
    if parts.is_empty() {
        "Repo".to_string()
    } else {
        parts.join(" ")
    }
}

fn repo_title_from_url(repo_url: &str) -> String {
    let fallback = humanize_repo_label(
        repo_url
            .rsplit('/')
            .next()
            .unwrap_or("repo")
            .trim_end_matches(".git"),
    );
    let Ok(parsed) = reqwest::Url::parse(repo_url) else {
        return fallback;
    };
    parsed
        .path_segments()
        .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
        .map(|segment| humanize_repo_label(segment.trim_end_matches(".git")))
        .unwrap_or(fallback)
}

fn build_repo_service_title(repo_title: &str, relative_dir: &str, kind: RepoServiceKind) -> String {
    if relative_dir.trim().is_empty() {
        return repo_title.to_string();
    }
    let segment = relative_dir
        .rsplit('/')
        .find(|part| !part.trim().is_empty())
        .unwrap_or(relative_dir);
    let label = humanize_repo_label(segment);
    if label.eq_ignore_ascii_case(kind.as_str()) {
        format!("{} {}", repo_title, label)
    } else {
        format!(
            "{} {} {}",
            repo_title,
            label,
            humanize_repo_label(kind.as_str())
        )
    }
}

fn repo_dir_name_hint(relative_dir: &str) -> Option<RepoServiceKind> {
    let lower = relative_dir.to_ascii_lowercase();
    let segment = lower
        .rsplit('/')
        .find(|part| !part.trim().is_empty())
        .unwrap_or(lower.as_str());
    if [
        "frontend",
        "front",
        "client",
        "web",
        "ui",
        "site",
        "app",
        "dashboard",
    ]
    .iter()
    .any(|needle| segment.contains(needle))
    {
        return Some(RepoServiceKind::Frontend);
    }
    if ["backend", "back", "api", "server", "svc", "service"]
        .iter()
        .any(|needle| segment.contains(needle))
    {
        return Some(RepoServiceKind::Backend);
    }
    None
}

fn is_allowed_repo_url(repo_url: &str) -> Result<reqwest::Url> {
    let parsed = reqwest::Url::parse(repo_url)
        .with_context(|| format!("invalid repo_url '{}'", repo_url))?;
    match parsed.scheme() {
        "https" | "http" => {}
        other => anyhow::bail!("unsupported repo_url scheme '{}': use http/https", other),
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("repo_url must include a host"))?;
    let lower_host = host.trim().to_ascii_lowercase();
    if lower_host == "localhost" || lower_host.ends_with(".local") {
        anyhow::bail!("repo_url must not target localhost or .local hosts");
    }
    if let Ok(ip) = lower_host.parse::<std::net::IpAddr>() {
        let blocked = match ip {
            std::net::IpAddr::V4(v4) => {
                v4.is_private() || v4.is_loopback() || v4.is_link_local() || v4.is_unspecified()
            }
            std::net::IpAddr::V6(v6) => {
                v6.is_loopback() || v6.is_unspecified() || v6.is_unique_local()
            }
        };
        if blocked {
            anyhow::bail!("repo_url must not target a private or loopback address");
        }
    }
    Ok(parsed)
}

fn should_skip_repo_dir(entry: &walkdir::DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return true;
    }
    let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
    !matches!(
        name.as_str(),
        ".git"
            | "node_modules"
            | ".next"
            | ".nuxt"
            | ".turbo"
            | "dist"
            | "build"
            | "coverage"
            | ".venv"
            | "venv"
            | "__pycache__"
            | "target"
            | ".idea"
            | ".vscode"
    )
}

fn read_text_file_limited(path: &Path, max_bytes: usize) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.is_empty() || bytes.len() > max_bytes {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn discover_readme_path(dir: &Path) -> Option<PathBuf> {
    for candidate in [
        "README.md",
        "README.MD",
        "README.txt",
        "README",
        "readme.md",
        "readme.txt",
        "readme",
    ] {
        let path = dir.join(candidate);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn normalize_readme_command_line(line: &str) -> Option<String> {
    let mut trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with("```")
        || trimmed.starts_with("<!--")
    {
        return None;
    }
    if let Some(stripped) = trimmed.strip_prefix("$ ") {
        trimmed = stripped.trim();
    }
    if let Some(stripped) = trimmed.strip_prefix("- ") {
        trimmed = stripped.trim();
    }
    if let Some(stripped) = trimmed.strip_prefix("* ") {
        trimmed = stripped.trim();
    }
    let trimmed = trimmed.trim_matches('`').trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn extract_readme_hints(readme: &str) -> RepoReadmeHints {
    let mut hints = RepoReadmeHints::default();
    for line in readme.lines() {
        let Some(command) = normalize_readme_command_line(line) else {
            continue;
        };
        let lower = command.to_ascii_lowercase();
        if lower.contains("docker compose") || lower.contains("docker-compose") {
            hints.mentions_compose = true;
        }
        if hints.install_command.is_none()
            && [
                "npm install",
                "npm ci",
                "pnpm install",
                "yarn install",
                "pip install",
                "poetry install",
                "uv sync",
                "cargo build",
            ]
            .iter()
            .any(|needle| lower.starts_with(needle))
        {
            hints.install_command = Some(command.clone());
        }
        if hints.start_command.is_none()
            && [
                "npm run dev",
                "npm run start",
                "pnpm dev",
                "pnpm start",
                "yarn dev",
                "yarn start",
                "uvicorn ",
                "python ",
                "streamlit run",
                "flask run",
                "cargo run",
                "docker compose up",
                "docker-compose up",
            ]
            .iter()
            .any(|needle| lower.starts_with(needle))
        {
            hints.start_command = Some(command);
        }
    }
    hints
}

fn load_readme_hints(dir: &Path) -> Option<(String, RepoReadmeHints)> {
    let path = discover_readme_path(dir)?;
    let content = read_text_file_limited(&path, MAX_README_BYTES)?;
    let relative = path
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "README".to_string());
    Some((relative, extract_readme_hints(&content)))
}

fn load_node_manifest(dir: &Path) -> Option<RepoNodeManifest> {
    let raw = read_text_file_limited(&dir.join("package.json"), MAX_REPO_TEXT_FILE_BYTES)?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let mut manifest = RepoNodeManifest {
        name: parsed
            .get("name")
            .and_then(|value| value.as_str())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        ..RepoNodeManifest::default()
    };
    if parsed.get("workspaces").is_some() {
        manifest.has_workspaces = true;
    }
    for key in [
        "scripts",
        "dependencies",
        "devDependencies",
        "optionalDependencies",
    ] {
        if let Some(obj) = parsed.get(key).and_then(|value| value.as_object()) {
            if key == "scripts" {
                manifest.scripts.extend(obj.keys().cloned());
            } else {
                manifest.dependencies.extend(
                    obj.keys()
                        .map(|value| value.to_ascii_lowercase())
                        .collect::<HashSet<_>>(),
                );
            }
        }
    }
    Some(manifest)
}

fn load_python_dependency_text(dir: &Path) -> String {
    let mut combined = String::new();
    for candidate in ["requirements.txt", "pyproject.toml"] {
        let path = dir.join(candidate);
        if let Some(text) = read_text_file_limited(&path, MAX_REPO_TEXT_FILE_BYTES) {
            if !combined.is_empty() {
                combined.push('\n');
            }
            combined.push_str(&text);
        }
    }
    combined
}

fn first_existing_file(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    names
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.exists())
}

fn build_relative_file_arg(relative_dir: &str, filename: &str) -> String {
    if relative_dir.trim().is_empty() {
        filename.to_string()
    } else {
        format!("{}/{}", relative_dir.trim_end_matches('/'), filename)
    }
}

fn detect_fastapi_entry(dir: &Path) -> Option<PathBuf> {
    for candidate in ["main.py", "app.py", "server.py", "api.py"] {
        let path = dir.join(candidate);
        let Some(text) = read_text_file_limited(&path, MAX_REPO_TEXT_FILE_BYTES) else {
            continue;
        };
        if text.contains("FastAPI(") || text.contains("from fastapi import") {
            return Some(path);
        }
    }
    None
}

fn detect_flask_entry(dir: &Path) -> Option<PathBuf> {
    for candidate in ["app.py", "main.py", "server.py", "wsgi.py"] {
        let path = dir.join(candidate);
        let Some(text) = read_text_file_limited(&path, MAX_REPO_TEXT_FILE_BYTES) else {
            continue;
        };
        if text.contains("Flask(") || text.contains("from flask import") {
            return Some(path);
        }
    }
    None
}

fn build_python_commands(
    dir: &Path,
    relative_dir: &str,
) -> Option<(RepoServiceKind, Option<String>, String)> {
    let dependency_text = load_python_dependency_text(dir).to_ascii_lowercase();
    let requirements_path = dir.join("requirements.txt");
    let pyproject_path = dir.join("pyproject.toml");
    let install_command = if requirements_path.exists() {
        Some(format!(
            "pip install -r {} -q",
            shell_quote_arg(&build_relative_file_arg(relative_dir, "requirements.txt"))
        ))
    } else if pyproject_path.exists() {
        Some(if relative_dir.trim().is_empty() {
            "pip install -e .".to_string()
        } else {
            format!("pip install -e {}", shell_quote_arg(relative_dir))
        })
    } else {
        None
    };

    if dir.join("manage.py").exists() {
        return Some((
            RepoServiceKind::Backend,
            install_command,
            format!(
                "python {} runserver 0.0.0.0:{{PORT}}",
                shell_quote_arg(&build_relative_file_arg(relative_dir, "manage.py"))
            ),
        ));
    }

    if dependency_text.contains("streamlit") {
        if let Some(entry) = first_existing_file(dir, &["app.py", "main.py", "streamlit_app.py"]) {
            let rel = normalize_repo_relative_path(entry.strip_prefix(dir).ok().unwrap_or(&entry));
            return Some((
                RepoServiceKind::Fullstack,
                install_command,
                format!(
                    "streamlit run {} --server.address 0.0.0.0 --server.port {{PORT}}",
                    shell_quote_arg(&build_relative_file_arg(relative_dir, &rel))
                ),
            ));
        }
    }

    if let Some(entry) = detect_fastapi_entry(dir) {
        let rel_dir = entry.parent().unwrap_or(dir);
        let app_dir = if rel_dir == dir {
            relative_dir.to_string()
        } else {
            let nested =
                normalize_repo_relative_path(rel_dir.strip_prefix(dir).ok().unwrap_or(rel_dir));
            if relative_dir.trim().is_empty() {
                nested
            } else if nested.is_empty() {
                relative_dir.to_string()
            } else {
                format!("{}/{}", relative_dir.trim_end_matches('/'), nested)
            }
        };
        let module = entry
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("app");
        let app_dir_arg = if app_dir.trim().is_empty() {
            ".".to_string()
        } else {
            app_dir
        };
        return Some((
            RepoServiceKind::Backend,
            install_command,
            format!(
                "uvicorn --app-dir {} {}:app --host 0.0.0.0 --port {{PORT}}",
                shell_quote_arg(&app_dir_arg),
                module
            ),
        ));
    }

    if let Some(entry) = detect_flask_entry(dir) {
        let rel = normalize_repo_relative_path(entry.strip_prefix(dir).ok().unwrap_or(&entry));
        return Some((
            RepoServiceKind::Backend,
            install_command,
            format!(
                "flask --app {} run --host 0.0.0.0 --port {{PORT}}",
                shell_quote_arg(&build_relative_file_arg(relative_dir, &rel))
            ),
        ));
    }

    if dependency_text.contains("gradio") {
        if let Some(entry) = first_existing_file(dir, &["app.py", "main.py"]) {
            let rel = normalize_repo_relative_path(entry.strip_prefix(dir).ok().unwrap_or(&entry));
            return Some((
                RepoServiceKind::Fullstack,
                install_command,
                format!(
                    "python {}",
                    shell_quote_arg(&build_relative_file_arg(relative_dir, &rel))
                ),
            ));
        }
    }

    if let Some(entry) = first_existing_file(dir, &["server.py", "app.py", "main.py", "run.py"]) {
        let rel = normalize_repo_relative_path(entry.strip_prefix(dir).ok().unwrap_or(&entry));
        return Some((
            repo_dir_name_hint(relative_dir).unwrap_or(RepoServiceKind::Backend),
            install_command,
            format!(
                "python {}",
                shell_quote_arg(&build_relative_file_arg(relative_dir, &rel))
            ),
        ));
    }

    None
}

fn classify_node_service_kind(manifest: &RepoNodeManifest, relative_dir: &str) -> RepoServiceKind {
    let deps = &manifest.dependencies;
    let has_frontend_framework = deps.iter().any(|dep| {
        matches!(
            dep.as_str(),
            "react"
                | "react-dom"
                | "vite"
                | "next"
                | "vue"
                | "nuxt"
                | "svelte"
                | "@sveltejs/kit"
                | "astro"
                | "gatsby"
                | "@angular/core"
                | "remix"
        )
    });
    let has_backend_framework = deps.iter().any(|dep| {
        matches!(
            dep.as_str(),
            "express" | "koa" | "fastify" | "hapi" | "@nestjs/core" | "@nestjs/common" | "restify"
        )
    });
    if deps.contains("next")
        || deps.contains("nuxt")
        || deps.contains("@sveltejs/kit")
        || deps.contains("remix")
    {
        return RepoServiceKind::Fullstack;
    }
    if has_frontend_framework && has_backend_framework {
        return RepoServiceKind::Fullstack;
    }
    if has_frontend_framework {
        return RepoServiceKind::Frontend;
    }
    if has_backend_framework {
        return RepoServiceKind::Backend;
    }
    repo_dir_name_hint(relative_dir).unwrap_or(RepoServiceKind::Backend)
}

fn build_node_run_command(
    manifest: &RepoNodeManifest,
    relative_dir: &str,
    script: &str,
    extra_args: &[&str],
    root_has_workspaces: bool,
) -> String {
    let workspace_name = manifest.name.as_deref().filter(|_| root_has_workspaces);
    let mut command = if let Some(name) = workspace_name {
        format!("npm run {} --workspace={}", script, shell_quote_arg(name))
    } else if relative_dir.trim().is_empty() {
        format!("npm run {}", script)
    } else {
        format!(
            "npm --prefix {} run {}",
            shell_quote_arg(relative_dir),
            script
        )
    };
    if !extra_args.is_empty() {
        command.push_str(" -- ");
        command.push_str(&extra_args.join(" "));
    }
    command
}

fn build_node_commands(
    dir: &Path,
    relative_dir: &str,
    manifest: &RepoNodeManifest,
    root_has_workspaces: bool,
) -> Option<(RepoServiceKind, String, String)> {
    let kind = classify_node_service_kind(manifest, relative_dir);
    let install_command = if relative_dir.trim().is_empty() || root_has_workspaces {
        "npm install --omit=dev".to_string()
    } else {
        format!(
            "npm --prefix {} install --omit=dev",
            shell_quote_arg(relative_dir)
        )
    };

    let frontend_args = if manifest.dependencies.contains("next") {
        vec!["--hostname", "0.0.0.0", "--port", "{PORT}"]
    } else {
        vec!["--host", "0.0.0.0", "--port", "{PORT}"]
    };

    let entry_command = if manifest.scripts.contains("preview")
        && matches!(kind, RepoServiceKind::Frontend | RepoServiceKind::Fullstack)
    {
        build_node_run_command(
            manifest,
            relative_dir,
            "preview",
            &frontend_args,
            root_has_workspaces,
        )
    } else if manifest.scripts.contains("start") {
        if manifest.dependencies.contains("next") {
            build_node_run_command(
                manifest,
                relative_dir,
                "start",
                &["--hostname", "0.0.0.0", "--port", "{PORT}"],
                root_has_workspaces,
            )
        } else {
            build_node_run_command(manifest, relative_dir, "start", &[], root_has_workspaces)
        }
    } else if manifest.scripts.contains("dev") {
        if matches!(kind, RepoServiceKind::Frontend | RepoServiceKind::Fullstack) {
            build_node_run_command(
                manifest,
                relative_dir,
                "dev",
                &frontend_args,
                root_has_workspaces,
            )
        } else {
            build_node_run_command(manifest, relative_dir, "dev", &[], root_has_workspaces)
        }
    } else if dir.join("server.js").exists() {
        let path = build_relative_file_arg(relative_dir, "server.js");
        format!("node {}", shell_quote_arg(&path))
    } else if dir.join("app.js").exists() {
        let path = build_relative_file_arg(relative_dir, "app.js");
        format!("node {}", shell_quote_arg(&path))
    } else if dir.join("index.js").exists() && matches!(kind, RepoServiceKind::Backend) {
        let path = build_relative_file_arg(relative_dir, "index.js");
        format!("node {}", shell_quote_arg(&path))
    } else {
        return None;
    };

    Some((kind, install_command, entry_command))
}

fn collect_env_example_inputs(scope_root: &Path) -> Vec<AppRequiredInput> {
    let mut out = Vec::new();
    for candidate in [
        ".env.example",
        ".env.sample",
        ".env.template",
        ".env.local.example",
        ".env.development.example",
    ] {
        let path = scope_root.join(candidate);
        let Some(text) = read_text_file_limited(&path, MAX_REPO_TEXT_FILE_BYTES) else {
            continue;
        };
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let Some((key, _)) = trimmed.split_once('=') else {
                continue;
            };
            let normalized = key.trim();
            if normalized.is_empty()
                || !normalized
                    .chars()
                    .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
            {
                continue;
            }
            let sensitive = ["KEY", "TOKEN", "SECRET", "PASSWORD", "PASS", "PRIVATE"]
                .iter()
                .any(|needle| normalized.contains(needle));
            push_required_input(&mut out, normalized, sensitive);
        }
    }
    out
}

fn discover_repo_candidate_dirs(root: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let walker = walkdir::WalkDir::new(root)
        .min_depth(0)
        .max_depth(3)
        .into_iter()
        .filter_entry(should_skip_repo_dir);
    for entry in walker.flatten() {
        if !entry.file_type().is_dir() {
            continue;
        }
        let dir = entry.path();
        let is_candidate = dir.join("package.json").exists()
            || dir.join("requirements.txt").exists()
            || dir.join("pyproject.toml").exists()
            || dir.join("manage.py").exists()
            || dir.join("index.html").exists();
        if is_candidate {
            candidates.push(dir.to_path_buf());
        }
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

fn plan_repo_services(
    repo_root: &Path,
    repo_title: &str,
    service_mode: RepoServiceMode,
) -> Result<Vec<RepoServicePlan>> {
    let root_manifest = load_node_manifest(repo_root);
    let root_has_workspaces = root_manifest
        .as_ref()
        .map(|manifest| manifest.has_workspaces)
        .unwrap_or(false);
    let root_readme_hints = load_readme_hints(repo_root);
    let candidate_dirs = discover_repo_candidate_dirs(repo_root);
    let has_child_package = candidate_dirs
        .iter()
        .any(|candidate| candidate != repo_root && candidate.join("package.json").exists());
    let mut plans = Vec::new();

    for dir in candidate_dirs {
        let relative_dir =
            normalize_repo_relative_path(dir.strip_prefix(repo_root).unwrap_or(&dir));
        if root_has_workspaces && relative_dir.is_empty() && has_child_package {
            continue;
        }

        let local_readme = load_readme_hints(&dir)
            .map(|(_, hints)| hints)
            .unwrap_or_default();
        let _readme_hints = if local_readme.install_command.is_some()
            || local_readme.start_command.is_some()
            || local_readme.mentions_compose
        {
            local_readme
        } else {
            root_readme_hints
                .as_ref()
                .map(|(_, hints)| hints.clone())
                .unwrap_or_default()
        };

        let required_inputs = {
            let mut inputs = collect_env_example_inputs(repo_root);
            for input in collect_env_example_inputs(&dir) {
                push_required_input(&mut inputs, &input.key, input.sensitive);
            }
            inputs
        };

        if let Some(manifest) = load_node_manifest(&dir) {
            let Some((kind, install_command, entry_command)) =
                build_node_commands(&dir, &relative_dir, &manifest, root_has_workspaces)
            else {
                continue;
            };
            if !kind.matches_mode(service_mode) {
                continue;
            }
            plans.push(RepoServicePlan {
                title: build_repo_service_title(repo_title, &relative_dir, kind),
                relative_dir,
                kind,
                copy_scope: RepoCopyScope::RepositoryRoot,
                install_command: Some(install_command),
                entry_command: Some(entry_command),
                required_inputs,
                detection_reason: "package.json scripts".to_string(),
            });
            continue;
        }

        if let Some((kind, install_command, entry_command)) =
            build_python_commands(&dir, &relative_dir)
        {
            if !kind.matches_mode(service_mode) {
                continue;
            }
            plans.push(RepoServicePlan {
                title: build_repo_service_title(repo_title, &relative_dir, kind),
                relative_dir,
                kind,
                copy_scope: RepoCopyScope::RepositoryRoot,
                install_command,
                entry_command: Some(entry_command),
                required_inputs,
                detection_reason: "python app manifest".to_string(),
            });
            continue;
        }

        if dir.join("index.html").exists() {
            let kind = RepoServiceKind::Static;
            if !kind.matches_mode(service_mode) {
                continue;
            }
            plans.push(RepoServicePlan {
                title: build_repo_service_title(repo_title, &relative_dir, kind),
                relative_dir,
                kind,
                copy_scope: RepoCopyScope::ServiceRoot,
                install_command: None,
                entry_command: None,
                required_inputs,
                detection_reason: "static index.html".to_string(),
            });
        }
    }

    if plans.is_empty() {
        if let Some((_, hints)) = root_readme_hints {
            if hints.mentions_compose {
                anyhow::bail!(
                    "Repo README suggests docker compose, but managed compose lifecycles are not supported yet. Use a repo with a directly runnable app or split the services explicitly."
                );
            }
            if service_mode == RepoServiceMode::Auto {
                if let Some(start_command) = hints.start_command {
                    plans.push(RepoServicePlan {
                        title: repo_title.to_string(),
                        relative_dir: String::new(),
                        kind: RepoServiceKind::Fullstack,
                        copy_scope: RepoCopyScope::RepositoryRoot,
                        install_command: hints.install_command,
                        entry_command: Some(start_command),
                        required_inputs: collect_env_example_inputs(repo_root),
                        detection_reason: "README install/run instructions".to_string(),
                    });
                }
            }
        }
    }

    if plans.len() > MAX_REPO_COMMAND_COUNT {
        anyhow::bail!(
            "Repo analysis detected too many runnable services ({}). Narrow the repo with repo_subdir or service_mode.",
            plans.len()
        );
    }
    Ok(plans)
}

fn collect_repo_files(root: &Path) -> Result<serde_json::Map<String, serde_json::Value>> {
    let mut files = serde_json::Map::new();
    let mut total_bytes = 0usize;
    let mut total_files = 0usize;
    let walker = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(should_skip_repo_dir);
    for entry in walker.flatten() {
        if entry.file_type().is_dir() {
            continue;
        }
        let path = entry.path();
        let metadata = match entry.metadata() {
            Ok(value) => value,
            Err(_) => continue,
        };
        if metadata.len() == 0 || metadata.len() as usize > MAX_REPO_TEXT_FILE_BYTES {
            continue;
        }
        let relative = normalize_repo_relative_path(path.strip_prefix(root).unwrap_or(path));
        if relative.is_empty() {
            continue;
        }
        let Some(content) = read_text_file_limited(path, MAX_REPO_TEXT_FILE_BYTES) else {
            continue;
        };
        total_files += 1;
        total_bytes += content.len();
        if total_files > MAX_REPO_TEXT_FILES {
            anyhow::bail!(
                "Repo is too large to deploy safely (>{} text files). Narrow it with repo_subdir.",
                MAX_REPO_TEXT_FILES
            );
        }
        if total_bytes > MAX_REPO_TOTAL_TEXT_BYTES {
            anyhow::bail!(
                "Repo is too large to deploy safely (>{} bytes of text content). Narrow it with repo_subdir.",
                MAX_REPO_TOTAL_TEXT_BYTES
            );
        }
        files.insert(relative, serde_json::Value::String(content));
    }
    if files.is_empty() {
        anyhow::bail!("Repo did not contain any deployable text files after filtering");
    }
    Ok(files)
}

async fn clone_repo(
    repo_url: &str,
    repo_ref: Option<&str>,
    target_dir: &Path,
    stream_tx: &Option<Sender<StreamEvent>>,
) -> Result<()> {
    let mut clone_args = vec!["git".to_string(), "clone".to_string()];
    if repo_ref.is_none() {
        clone_args.push("--depth".to_string());
        clone_args.push("1".to_string());
    }
    clone_args.push(repo_url.to_string());
    clone_args.push(target_dir.to_string_lossy().to_string());

    emit_progress(stream_tx, &format!("Cloning repository {}", repo_url)).await;
    let output = run_local_command_with_progress(
        &join_shell_command(&clone_args),
        "git clone",
        std::env::current_dir()?.as_path(),
        &HashMap::new(),
        MAX_REPO_CLONE_TIMEOUT_SECS,
        stream_tx,
        "repo_clone",
    )
    .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else {
            stdout.trim().to_string()
        };
        anyhow::bail!("git clone failed: {}", detail);
    }

    if let Some(reference) = repo_ref.filter(|value| !value.trim().is_empty()) {
        emit_progress(stream_tx, &format!("Checking out repo ref {}", reference)).await;
        let output = run_local_command_with_progress(
            &format!("git checkout {}", shell_quote_arg(reference)),
            "git checkout",
            target_dir,
            &HashMap::new(),
            120,
            stream_tx,
            "repo_checkout",
        )
        .await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let detail = if !stderr.trim().is_empty() {
                stderr.trim().to_string()
            } else {
                stdout.trim().to_string()
            };
            anyhow::bail!("git checkout failed: {}", detail);
        }
    }

    Ok(())
}

async fn deploy_repo_bundle(
    config_dir: &Path,
    data_dir: &Path,
    arguments: &serde_json::Value,
    registry: &AppRegistry,
    llm_env: &HashMap<String, String>,
    stream_tx: Option<Sender<StreamEvent>>,
) -> Result<String> {
    let repo_url = arguments
        .get("repo_url")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("repo_url cannot be empty"))?;
    let parsed_url = is_allowed_repo_url(repo_url)?;
    let repo_ref = arguments
        .get("repo_ref")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let repo_subdir = arguments
        .get("repo_subdir")
        .and_then(|value| value.as_str())
        .map(|value| value.trim_matches('/').trim_matches('\\'))
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let requested_title = arguments
        .get("title")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let repo_title = requested_title
        .map(|value| value.to_string())
        .unwrap_or_else(|| repo_title_from_url(parsed_url.as_str()));
    let service_mode = repo_service_mode_from_opt(
        arguments
            .get("service_mode")
            .and_then(|value| value.as_str()),
    );
    let runtime_preference = if arguments
        .get("runtime_preference")
        .and_then(|value| value.as_str())
        .is_some()
    {
        runtime_preference_from_opt(
            arguments
                .get("runtime_preference")
                .and_then(|value| value.as_str()),
        )
    } else {
        RuntimePreference::Container
    };
    let expose_public = arguments
        .get("expose_public")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let access_guard_enabled = arguments
        .get("access_guard")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let runtime_image = arguments.get("runtime_image").cloned();

    let bundle_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let bundle_dir = data_dir.join("repo-deployments").join(&bundle_id);
    let source_dir = bundle_dir.join("source");
    tokio::fs::create_dir_all(&bundle_dir).await?;
    clone_repo(repo_url, repo_ref, &source_dir, &stream_tx).await?;

    let repo_root = if let Some(subdir) = repo_subdir.as_ref() {
        let candidate = source_dir.join(subdir);
        if !candidate.exists() || !candidate.is_dir() {
            anyhow::bail!(
                "repo_subdir '{}' was not found inside the cloned repo",
                subdir
            );
        }
        candidate
    } else {
        source_dir.clone()
    };

    let (readme_file, readme_mentions_compose) = load_readme_hints(&repo_root)
        .map(|(file, hints)| (Some(file), hints.mentions_compose))
        .unwrap_or((None, false));

    emit_progress(&stream_tx, "Reading repo README and local manifests").await;
    let service_plans = plan_repo_services(&repo_root, &repo_title, service_mode)?;
    if service_plans.is_empty() {
        anyhow::bail!(
            "I cloned the repo, but I could not detect a runnable frontend/backend service from the README or local manifests."
        );
    }
    emit_progress(
        &stream_tx,
        &format!(
            "Detected {} repo service(s): {}",
            service_plans.len(),
            service_plans
                .iter()
                .map(|plan| format!(
                    "{} ({})",
                    if plan.relative_dir.is_empty() {
                        ".".to_string()
                    } else {
                        plan.relative_dir.clone()
                    },
                    plan.kind.as_str()
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    )
    .await;

    let mut deployed_services = Vec::new();
    let mut success_like_count = 0usize;
    let mut needs_inputs_count = 0usize;
    let mut failure_count = 0usize;

    for (idx, plan) in service_plans.iter().enumerate() {
        emit_progress(
            &stream_tx,
            &format!(
                "Deploying repo service {}/{}: {}",
                idx + 1,
                service_plans.len(),
                plan.title
            ),
        )
        .await;
        let scope_root = match plan.copy_scope {
            RepoCopyScope::RepositoryRoot => &repo_root,
            RepoCopyScope::ServiceRoot => {
                if plan.relative_dir.is_empty() {
                    &repo_root
                } else {
                    &repo_root.join(&plan.relative_dir)
                }
            }
        };
        let files = collect_repo_files(scope_root)?;
        let mut service_args = serde_json::Map::new();
        service_args.insert("files".to_string(), serde_json::Value::Object(files));
        service_args.insert("title".to_string(), serde_json::json!(plan.title));
        service_args.insert(
            "runtime_preference".to_string(),
            serde_json::json!(runtime_preference.as_str()),
        );
        service_args.insert(
            "expose_public".to_string(),
            serde_json::json!(expose_public),
        );
        service_args.insert(
            "access_guard".to_string(),
            serde_json::json!(access_guard_enabled),
        );
        service_args.insert("repo_url".to_string(), serde_json::json!(repo_url));
        service_args.insert("repo_bundle_id".to_string(), serde_json::json!(bundle_id));
        service_args.insert(
            "repo_service_kind".to_string(),
            serde_json::json!(plan.kind.as_str()),
        );
        service_args.insert(
            "repo_service_dir".to_string(),
            serde_json::json!(plan.relative_dir),
        );
        if let Some(ref value) = repo_ref {
            service_args.insert("repo_ref".to_string(), serde_json::json!(value));
        }
        if let Some(ref value) = repo_subdir {
            service_args.insert("repo_subdir".to_string(), serde_json::json!(value));
        }
        if let Some(ref value) = runtime_image {
            service_args.insert("runtime_image".to_string(), value.clone());
        }
        if !plan.required_inputs.is_empty() {
            service_args.insert(
                "required_inputs".to_string(),
                serde_json::to_value(&plan.required_inputs)
                    .unwrap_or_else(|_| serde_json::json!([])),
            );
        }
        if let Some(command) = plan.install_command.as_ref() {
            service_args.insert("install_command".to_string(), serde_json::json!(command));
        }
        if let Some(command) = plan.entry_command.as_ref() {
            service_args.insert("entry_command".to_string(), serde_json::json!(command));
        }

        match std::pin::Pin::from(Box::new(app_deploy(
            config_dir,
            data_dir,
            &serde_json::Value::Object(service_args),
            registry,
            llm_env,
            stream_tx.clone(),
        )))
        .await
        {
            Ok(result) => {
                let parsed = serde_json::from_str::<serde_json::Value>(&result)
                    .unwrap_or_else(|_| serde_json::json!({ "status": "deployed", "raw": result }));
                let status = parsed
                    .get("status")
                    .and_then(|value| value.as_str())
                    .unwrap_or("deployed");
                if matches!(status, "deployed" | "needs_secrets") {
                    success_like_count += 1;
                }
                if status == "needs_secrets" {
                    needs_inputs_count += 1;
                }
                deployed_services.push(serde_json::json!({
                    "title": plan.title,
                    "relative_dir": plan.relative_dir,
                    "kind": plan.kind.as_str(),
                    "status": status,
                    "detection_reason": plan.detection_reason,
                    "result": parsed,
                }));
            }
            Err(error) => {
                failure_count += 1;
                deployed_services.push(serde_json::json!({
                    "title": plan.title,
                    "relative_dir": plan.relative_dir,
                    "kind": plan.kind.as_str(),
                    "status": "failed",
                    "detection_reason": plan.detection_reason,
                    "error": error.to_string(),
                }));
            }
        }
    }

    let summary_status = if failure_count == 0 && needs_inputs_count == 0 {
        "deployed"
    } else if failure_count == 0 {
        "needs_inputs"
    } else if success_like_count > 0 {
        "deployed_partially"
    } else {
        anyhow::bail!(
            "Repo deploy failed for all detected services: {}",
            deployed_services
                .iter()
                .filter_map(|service| {
                    let title = service.get("title").and_then(|value| value.as_str())?;
                    let error = service.get("error").and_then(|value| value.as_str())?;
                    Some(format!("{}: {}", title, error))
                })
                .collect::<Vec<_>>()
                .join(" | ")
        );
    };

    let manifest = serde_json::json!({
        "bundle_id": bundle_id,
        "repo_url": repo_url,
        "repo_ref": repo_ref,
        "repo_subdir": repo_subdir,
        "title": repo_title,
        "service_mode": match service_mode {
            RepoServiceMode::Auto => "auto",
            RepoServiceMode::Frontend => "frontend",
            RepoServiceMode::Backend => "backend",
            RepoServiceMode::Fullstack => "fullstack",
        },
        "status": summary_status,
        "readme_file": readme_file,
        "readme_mentions_compose": readme_mentions_compose,
        "created_at": chrono::Utc::now().to_rfc3339(),
        "services": deployed_services,
    });
    tokio::fs::write(
        bundle_dir.join("bundle.json"),
        serde_json::to_string_pretty(&manifest)?,
    )
    .await?;
    let _ = tokio::fs::remove_dir_all(&source_dir).await;

    Ok(serde_json::json!({
        "status": summary_status,
        "deployment_kind": "repo_bundle",
        "bundle_id": bundle_id,
        "title": repo_title,
        "repo_url": repo_url,
        "repo_ref": repo_ref,
        "repo_subdir": repo_subdir,
        "readme_file": readme_file,
        "runtime_preference": runtime_preference.as_str(),
        "service_count": deployed_services.len(),
        "deployed_count": success_like_count,
        "failed_count": failure_count,
        "services": deployed_services,
    })
    .to_string())
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
    let normalized_env = env.trim().to_ascii_uppercase();
    let auto_llm_passthrough = matches!(
        normalized_env.as_str(),
        "OPENAI_API_KEY"
            | "OPENROUTER_API_KEY"
            | "ANTHROPIC_API_KEY"
            | "OPENAI_BASE_URL"
            | "OLLAMA_BASE_URL"
            | "LLM_MODEL"
            | "LLM_PROVIDER"
            | "API_KEY"
            | "LLM_API_KEY"
            | "MODEL_API_KEY"
            | "OPENAI_KEY"
            | "OPENAI_TOKEN"
            | "OPENROUTER_KEY"
            | "ANTHROPIC_KEY"
            | "CLAUDE_API_KEY"
    );
    if allow_llm_env_passthrough || auto_llm_passthrough {
        if let Some(v) = llm_env.get(env) {
            if !v.trim().is_empty() {
                return Some(v.clone());
            }
        }
        if let Some(v) = llm_env.get(normalized_env.as_str()) {
            if !v.trim().is_empty() {
                return Some(v.clone());
            }
        }
        // Common aliases should map to the active model key.
        match normalized_env.as_str() {
            "API_KEY" | "LLM_API_KEY" | "MODEL_API_KEY" | "OPENAI_KEY" | "OPENAI_TOKEN" => llm_env
                .get("OPENAI_API_KEY")
                .filter(|v| !v.trim().is_empty())
                .cloned()
                .or_else(|| {
                    llm_env
                        .get("ANTHROPIC_API_KEY")
                        .filter(|v| !v.trim().is_empty())
                        .cloned()
                })
                .or_else(|| {
                    llm_env
                        .get("OPENROUTER_API_KEY")
                        .filter(|v| !v.trim().is_empty())
                        .cloned()
                }),
            "OPENROUTER_KEY" => llm_env
                .get("OPENROUTER_API_KEY")
                .filter(|v| !v.trim().is_empty())
                .cloned(),
            "ANTHROPIC_KEY" | "CLAUDE_API_KEY" => llm_env
                .get("ANTHROPIC_API_KEY")
                .filter(|v| !v.trim().is_empty())
                .cloned(),
            _ => None,
        }
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
    let secrets = mgr.load_secrets()?;
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

fn command_looks_python_related(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    [
        "python",
        "pip",
        "uvicorn",
        "gunicorn",
        "streamlit",
        "flask",
        "django",
        "manage.py",
        "fastapi",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn local_runtime_stdout_log_path(app_dir: &Path) -> PathBuf {
    app_dir.join(LOCAL_RUNTIME_STDOUT_LOG_FILE)
}

fn local_runtime_stderr_log_path(app_dir: &Path) -> PathBuf {
    app_dir.join(LOCAL_RUNTIME_STDERR_LOG_FILE)
}

fn prepare_local_runtime_log_files(app_dir: &Path) -> Result<(PathBuf, PathBuf)> {
    let stdout_path = local_runtime_stdout_log_path(app_dir);
    let stderr_path = local_runtime_stderr_log_path(app_dir);

    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&stdout_path)
        .with_context(|| format!("failed to prepare runtime stdout log at {:?}", stdout_path))?;
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&stderr_path)
        .with_context(|| format!("failed to prepare runtime stderr log at {:?}", stderr_path))?;

    Ok((stdout_path, stderr_path))
}

fn open_local_runtime_log_for_append(path: &Path, label: &str) -> Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open runtime {} log at {:?}", label, path))
}

async fn read_file_tail(path: &Path, max_bytes: usize) -> String {
    let Ok(bytes) = tokio::fs::read(path).await else {
        return String::new();
    };
    if bytes.is_empty() {
        return String::new();
    }
    let start = bytes.len().saturating_sub(max_bytes);
    String::from_utf8_lossy(&bytes[start..]).trim().to_string()
}

pub async fn read_local_runtime_log_tail(app_dir: &Path, max_bytes: usize) -> String {
    let stderr_tail = read_file_tail(&local_runtime_stderr_log_path(app_dir), max_bytes).await;
    let stdout_tail = read_file_tail(&local_runtime_stdout_log_path(app_dir), max_bytes).await;
    let mut parts = Vec::new();
    if !stderr_tail.is_empty() {
        parts.push(format!("stderr:\n{}", stderr_tail));
    }
    if !stdout_tail.is_empty() {
        parts.push(format!("stdout:\n{}", stdout_tail));
    }
    parts.join("\n\n")
}

fn prepend_path_entry(prefix: &Path, existing_path: Option<&str>) -> Option<String> {
    let mut entries: Vec<PathBuf> = vec![prefix.to_path_buf()];
    if let Some(existing) = existing_path {
        entries.extend(std::env::split_paths(existing));
    } else if let Some(system) = std::env::var_os("PATH") {
        entries.extend(std::env::split_paths(&system));
    }
    std::env::join_paths(entries)
        .ok()
        .and_then(|v| v.into_string().ok())
}

async fn ensure_local_python_venv(app_dir: &Path) -> Result<(PathBuf, PathBuf)> {
    let venv_dir = app_dir.join(".venv");
    let bin_dir = if cfg!(windows) {
        venv_dir.join("Scripts")
    } else {
        venv_dir.join("bin")
    };
    let python_candidates = if cfg!(windows) {
        vec![bin_dir.join("python.exe"), bin_dir.join("python")]
    } else {
        vec![bin_dir.join("python3"), bin_dir.join("python")]
    };

    if python_candidates.iter().any(|p| p.exists()) {
        return Ok((venv_dir, bin_dir));
    }

    let creators: Vec<(&str, Vec<&str>)> = if cfg!(windows) {
        vec![
            ("python", vec!["-m", "venv", ".venv"]),
            ("py", vec!["-3", "-m", "venv", ".venv"]),
        ]
    } else {
        vec![
            ("python3", vec!["-m", "venv", ".venv"]),
            ("python", vec!["-m", "venv", ".venv"]),
        ]
    };

    let mut last_error = String::new();
    for (program, args) in creators {
        let mut cmd = tokio::process::Command::new(program);
        cmd.args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .current_dir(app_dir);
        match tokio::time::timeout(std::time::Duration::from_secs(120), cmd.output()).await {
            Ok(Ok(output)) if output.status.success() => {
                if python_candidates.iter().any(|p| p.exists()) {
                    return Ok((venv_dir, bin_dir));
                }
                last_error = "venv command succeeded but Python executable was not found in .venv"
                    .to_string();
            }
            Ok(Ok(output)) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                last_error = if !stderr.trim().is_empty() {
                    stderr.trim().to_string()
                } else if !stdout.trim().is_empty() {
                    stdout.trim().to_string()
                } else {
                    format!("{} -m venv exited with status {}", program, output.status)
                };
            }
            Ok(Err(e)) => {
                last_error = format!("failed to spawn {}: {}", program, e);
            }
            Err(_) => {
                last_error = format!("{} -m venv timed out", program);
            }
        }
    }

    if last_error.is_empty() {
        last_error = "unknown error creating .venv".to_string();
    }
    anyhow::bail!(
        "failed to prepare local Python virtual environment: {}",
        last_error
    );
}

/// Validate and normalise an app entry command.
/// If the command contains shell operators (`&&`, `|`, `;`, etc.), wrap it in
/// `sh -c "..."` so it runs through a shell interpreter inside the sandbox.
fn validate_app_command(command: &str, label: &str) -> Result<String> {
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
    // Collapse multi-line commands into a single line joined with &&
    let collapsed = if trimmed.contains('\n') || trimmed.contains('\r') {
        trimmed
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join(" && ")
    } else {
        trimmed.to_string()
    };
    let lowered = collapsed.to_ascii_lowercase();
    if lowered.starts_with("sh -c ") || lowered.starts_with("bash -c ") {
        return Ok(collapsed);
    }

    let shell_tokens = ["&&", "||", ";", "|", "`", "$(", "<", ">"];
    if shell_tokens.iter().any(|tok| collapsed.contains(tok)) {
        // Wrap in sh -c so shell operators work inside the sandbox
        let escaped = collapsed
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('$', "\\$")
            .replace('`', "\\`");
        Ok(format!("sh -c \"{}\"", escaped))
    } else {
        Ok(collapsed)
    }
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
            anyhow::bail!(
                "Env value for '{}' contains unsupported control characters",
                k
            );
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

/// Rewrite absolute `/app/` paths in entry commands to be relative to `app_dir`.
/// Container-authored commands use `/app/server.py` etc., but when running as a
/// local process the cwd is already `app_dir`, so `/app/server.py` should become
/// `./server.py` (or the actual file in `app_dir`).
fn localize_app_entry_command(command: &str, app_dir: &Path) -> String {
    let mut parts: Vec<String> = command.split_whitespace().map(|s| s.to_string()).collect();
    for part in &mut parts {
        if part.starts_with("/app/") {
            let relative = &part[5..]; // strip "/app/"
            let candidate = app_dir.join(relative);
            if candidate.exists() {
                *part = format!("./{}", relative);
            }
        }
    }
    parts.join(" ")
}

fn split_command_args(command: &str, label: &str) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escape = false;

    for ch in command.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }
        match quote {
            Some(q) => {
                if ch == '\\' && q == '"' {
                    escape = true;
                } else if ch == q {
                    quote = None;
                } else {
                    current.push(ch);
                }
            }
            None => {
                if ch == '\'' || ch == '"' {
                    quote = Some(ch);
                } else if ch == '\\' {
                    escape = true;
                } else if ch.is_whitespace() {
                    if !current.is_empty() {
                        out.push(std::mem::take(&mut current));
                    }
                } else {
                    current.push(ch);
                }
            }
        }
    }

    if escape {
        anyhow::bail!("{} has a trailing escape character", label);
    }
    if quote.is_some() {
        anyhow::bail!("{} has an unclosed quote", label);
    }
    if !current.is_empty() {
        out.push(current);
    }
    if out.is_empty() {
        anyhow::bail!("{} cannot be empty", label);
    }
    Ok(out)
}

fn shell_quote_arg(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }
    let safe = arg.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || matches!(
                c,
                '_' | '-' | '.' | '/' | ':' | '@' | '%' | '+' | '=' | ',' | '{' | '}'
            )
    });
    if safe {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\"'\"'"))
    }
}

fn join_shell_command(args: &[String]) -> String {
    args.iter()
        .map(|arg| shell_quote_arg(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_python_runtime_command_for_container(command: &str) -> String {
    let Ok(args) = split_command_args(command, "command") else {
        return command.to_string();
    };
    if args.len() >= 2 {
        let head = args[0].to_ascii_lowercase();
        if (head == "sh" || head == "bash") && args[1] == "-c" {
            return command.to_string();
        }
    }

    let candidates = command_arg_candidates(&args);
    if candidates.is_empty() {
        return command.to_string();
    }

    if let Some(py3) = candidates.iter().find(|candidate| {
        candidate
            .first()
            .is_some_and(|p| p.eq_ignore_ascii_case("python3"))
    }) {
        return join_shell_command(py3);
    }
    join_shell_command(candidates.first().unwrap_or(&args))
}

fn command_arg_candidates(args: &[String]) -> Vec<Vec<String>> {
    if args.is_empty() {
        return Vec::new();
    }
    let program = args[0].trim();
    if program.is_empty() {
        return vec![args.to_vec()];
    }
    let has_path_hint = program.contains('/') || program.contains('\\');
    if has_path_hint {
        return vec![args.to_vec()];
    }

    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let mut candidates: Vec<Vec<String>> = vec![args.to_vec()];
    let lowered = program.to_ascii_lowercase();

    let push_program_variant = |list: &mut Vec<Vec<String>>, alt: &str| {
        let mut variant = Vec::with_capacity(1 + rest.len());
        variant.push(alt.to_string());
        variant.extend(rest.iter().cloned());
        list.push(variant);
    };
    let push_module_variant = |list: &mut Vec<Vec<String>>, py: &str, module: &str| {
        let mut variant = Vec::with_capacity(3 + rest.len());
        variant.push(py.to_string());
        variant.push("-m".to_string());
        variant.push(module.to_string());
        variant.extend(rest.iter().cloned());
        list.push(variant);
    };

    match lowered.as_str() {
        "python" => {
            push_program_variant(&mut candidates, "python3");
            if cfg!(windows) {
                push_program_variant(&mut candidates, "py");
            }
        }
        "python3" => {
            push_program_variant(&mut candidates, "python");
        }
        "pip" => {
            push_program_variant(&mut candidates, "pip3");
            push_module_variant(&mut candidates, "python", "pip");
            push_module_variant(&mut candidates, "python3", "pip");
        }
        "pip3" => {
            push_program_variant(&mut candidates, "pip");
            push_module_variant(&mut candidates, "python3", "pip");
            push_module_variant(&mut candidates, "python", "pip");
        }
        "node" => {
            push_program_variant(&mut candidates, "nodejs");
        }
        "nodejs" => {
            push_program_variant(&mut candidates, "node");
        }
        "uvicorn" | "gunicorn" | "streamlit" | "flask" => {
            push_module_variant(&mut candidates, "python", lowered.as_str());
            push_module_variant(&mut candidates, "python3", lowered.as_str());
        }
        _ => {}
    }

    let mut deduped: Vec<Vec<String>> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for candidate in candidates {
        if candidate.is_empty() {
            continue;
        }
        let key = candidate.join("\u{1f}");
        if seen.insert(key) {
            deduped.push(candidate);
        }
    }
    deduped
}

async fn spawn_local_process_with_fallback(
    args: &[String],
    label: &str,
    cwd: &Path,
    envs: &HashMap<String, String>,
    stdout_log_path: &Path,
    stderr_log_path: &Path,
) -> Result<(tokio::process::Child, String)> {
    let mut attempted: Vec<String> = Vec::new();
    for candidate in command_arg_candidates(args) {
        if candidate.is_empty() {
            continue;
        }
        let program = candidate[0].clone();
        attempted.push(candidate.join(" "));
        let stdout_log = open_local_runtime_log_for_append(stdout_log_path, "stdout")?;
        let stderr_log = open_local_runtime_log_for_append(stderr_log_path, "stderr")?;
        let mut cmd = tokio::process::Command::new(&program);
        cmd.args(candidate.iter().skip(1))
            .stdout(Stdio::from(stdout_log))
            .stderr(Stdio::from(stderr_log))
            .kill_on_drop(true)
            .current_dir(cwd)
            .envs(envs);
        match cmd.spawn() {
            Ok(child) => return Ok((child, program)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => anyhow::bail!("failed to execute {} '{}': {}", label, program, e),
        }
    }
    anyhow::bail!(
        "failed to execute {}: no executable found (tried: {})",
        label,
        attempted.join(" | ")
    )
}

fn docker_required() -> bool {
    std::env::var("AGENTARK_APP_REQUIRE_DOCKER")
        .map(|v| {
            let normalized = v.trim().to_ascii_lowercase();
            normalized == "1" || normalized == "true" || normalized == "yes" || normalized == "on"
        })
        .unwrap_or(false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimePreference {
    Local,
    Container,
}

impl RuntimePreference {
    pub fn as_str(self) -> &'static str {
        match self {
            RuntimePreference::Local => "local",
            RuntimePreference::Container => "container",
        }
    }
}

fn default_runtime_preference() -> RuntimePreference {
    match std::env::var("AGENTARK_APP_RUNTIME_DEFAULT")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "container" | "docker" => RuntimePreference::Container,
        _ => RuntimePreference::Local,
    }
}

pub fn runtime_preference_from_opt(raw: Option<&str>) -> RuntimePreference {
    match raw.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "local" | "native" | "process" => RuntimePreference::Local,
        "container" | "docker" => RuntimePreference::Container,
        _ => default_runtime_preference(),
    }
}

fn with_node_bin_path(app_dir: &Path) -> Option<String> {
    let node_bin = app_dir.join("node_modules").join(".bin");
    if !node_bin.exists() {
        return None;
    }
    let mut entries: Vec<std::path::PathBuf> = vec![node_bin];
    if let Some(existing) = std::env::var_os("PATH") {
        entries.extend(std::env::split_paths(&existing));
    }
    std::env::join_paths(entries)
        .ok()
        .and_then(|os| os.into_string().ok())
}

fn compact_progress_line(line: &str, max_chars: usize) -> String {
    let trimmed = line.trim().replace('\r', "");
    let char_count = trimmed.chars().count();
    if char_count <= max_chars {
        return trimmed;
    }
    let head = max_chars.saturating_sub(3);
    format!("{}...", trimmed.chars().take(head).collect::<String>())
}

async fn run_local_command_with_progress(
    command: &str,
    label: &str,
    cwd: &Path,
    envs: &HashMap<String, String>,
    timeout_secs: u64,
    stream_tx: &Option<Sender<StreamEvent>>,
    stage: &str,
) -> Result<std::process::Output> {
    let args = split_command_args(command, label)?;
    let mut attempted: Vec<String> = Vec::new();
    for candidate in command_arg_candidates(&args) {
        if candidate.is_empty() {
            continue;
        }
        let program = candidate[0].clone();
        attempted.push(candidate.join(" "));
        let mut cmd = tokio::process::Command::new(&program);
        cmd.args(candidate.iter().skip(1))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .current_dir(cwd)
            .envs(envs);

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => anyhow::bail!("failed to execute {} '{}': {}", label, program, e),
        };

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stdout_tx = stream_tx.clone();
        let stderr_tx = stream_tx.clone();
        let stage_stdout = stage.to_string();
        let stage_stderr = stage.to_string();

        let stdout_task = tokio::spawn(async move {
            let mut collected = Vec::new();
            if let Some(stdout) = stdout {
                let mut lines = tokio::io::BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if line.trim().is_empty() {
                        continue;
                    }
                    collected.extend_from_slice(line.as_bytes());
                    collected.push(b'\n');
                    if let Some(tx) = stdout_tx.as_ref() {
                        let _ = tx
                            .send(StreamEvent::ToolProgress {
                                name: "app_deploy".to_string(),
                                content: format!(
                                    "{}: {}",
                                    stage_stdout,
                                    compact_progress_line(&line, 220)
                                ),
                                payload: None,
                            })
                            .await;
                    }
                }
            }
            collected
        });

        let stderr_task = tokio::spawn(async move {
            let mut collected = Vec::new();
            if let Some(stderr) = stderr {
                let mut lines = tokio::io::BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if line.trim().is_empty() {
                        continue;
                    }
                    collected.extend_from_slice(line.as_bytes());
                    collected.push(b'\n');
                    if let Some(tx) = stderr_tx.as_ref() {
                        let _ = tx
                            .send(StreamEvent::ToolProgress {
                                name: "app_deploy".to_string(),
                                content: format!(
                                    "{}: {}",
                                    stage_stderr,
                                    compact_progress_line(&line, 220)
                                ),
                                payload: None,
                            })
                            .await;
                    }
                }
            }
            collected
        });

        let status =
            match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), child.wait())
                .await
            {
                Err(_) => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    let _ = stdout_task.await;
                    let _ = stderr_task.await;
                    anyhow::bail!("{} timed out", label);
                }
                Ok(Ok(status)) => status,
                Ok(Err(e)) => {
                    let _ = stdout_task.await;
                    let _ = stderr_task.await;
                    anyhow::bail!("failed waiting for {} '{}': {}", label, program, e);
                }
            };

        let stdout = stdout_task.await.unwrap_or_default();
        let stderr = stderr_task.await.unwrap_or_default();
        return Ok(std::process::Output {
            status,
            stdout,
            stderr,
        });
    }

    anyhow::bail!(
        "failed to execute {}: no executable found (tried: {})",
        label,
        attempted.join(" | ")
    );
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

    let mut entry_cmd = validate_app_command(entry_command, "entry_command")?;
    let install_cmd = if let Some(cmd) = install_command {
        Some(validate_app_command(cmd, "install_command")?)
    } else {
        None
    };
    let uses_python_runtime = command_looks_python_related(&entry_cmd)
        || install_cmd
            .as_deref()
            .map(command_looks_python_related)
            .unwrap_or(false);
    if uses_python_runtime {
        entry_cmd = normalize_python_runtime_command_for_container(&entry_cmd);
    }

    let mut script_parts: Vec<String> = Vec::new();
    script_parts.push("set -e".to_string());
    script_parts.push("export PATH=\"/workspace/node_modules/.bin:$PATH\"".to_string());
    script_parts
        .push("export PYTHONPATH=\"/workspace/_deps${PYTHONPATH:+:$PYTHONPATH}\"".to_string());
    if uses_python_runtime {
        script_parts.push(
            "if [ ! -x /workspace/.venv/bin/python ]; then python3 -m venv /workspace/.venv || python -m venv /workspace/.venv || true; fi".to_string(),
        );
        script_parts.push(
            "if [ -x /workspace/.venv/bin/python ]; then . /workspace/.venv/bin/activate; fi"
                .to_string(),
        );
        script_parts.push("export PIP_DISABLE_PIP_VERSION_CHECK=1".to_string());
        script_parts.push("export PIP_BREAK_SYSTEM_PACKAGES=1".to_string());
    }
    if let Some(ref cmd) = install_cmd {
        let trimmed = cmd.trim();
        if !trimmed.is_empty() {
            let normalized = if uses_python_runtime {
                normalize_python_runtime_command_for_container(trimmed)
            } else {
                trimmed.to_string()
            };
            script_parts.push(normalized);
        }
    }
    script_parts.push(entry_cmd.trim().to_string());
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

pub async fn launch_dynamic_process(
    app_id: &str,
    app_dir: &Path,
    entry_command: &str,
    install_command: Option<&str>,
    port: u16,
    extra_env: &HashMap<String, String>,
    stream_tx: Option<Sender<StreamEvent>>,
) -> Result<tokio::process::Child> {
    // Normalize absolute /app/ paths to relative — entry commands are often authored
    // for container context where the app lives at /app/, but for local process runtime
    // the cwd is already app_dir so these should be relative.
    let entry_command = validate_app_command(
        &localize_app_entry_command(entry_command, app_dir).replace("{PORT}", &port.to_string()),
        "entry_command",
    )?;

    let install_command = if let Some(cmd) = install_command {
        Some(validate_app_command(
            &cmd.replace("{PORT}", &port.to_string()),
            "install_command",
        )?)
    } else {
        None
    };

    let mut runtime_env: HashMap<String, String> = HashMap::new();
    runtime_env.insert("PORT".to_string(), port.to_string());
    runtime_env.insert("HOST".to_string(), "0.0.0.0".to_string());
    runtime_env.extend(extra_env.clone());

    // If a per-app venv exists, prepend its bin to PATH so the entry command
    // picks up the venv's Python and installed packages automatically.
    let venv_dir = app_dir.join(".venv");
    if venv_dir.join("bin").exists() {
        let venv_bin = venv_dir.join("bin").to_string_lossy().to_string();
        let merged = std::env::var("PATH")
            .map(|existing| format!("{}:{}", venv_bin, existing))
            .unwrap_or(venv_bin.clone());
        runtime_env.insert("PATH".to_string(), merged);
        runtime_env.insert(
            "VIRTUAL_ENV".to_string(),
            venv_dir.to_string_lossy().to_string(),
        );
    }
    // Legacy: support old --target _deps installs for backward compat.
    let deps_dir = app_dir.join("_deps");
    if deps_dir.exists() {
        let deps = deps_dir.to_string_lossy().to_string();
        let merged = std::env::var("PYTHONPATH")
            .map(|existing| {
                if existing.trim().is_empty() {
                    deps.clone()
                } else if cfg!(windows) {
                    format!("{};{}", deps, existing)
                } else {
                    format!("{}:{}", deps, existing)
                }
            })
            .unwrap_or(deps);
        runtime_env.insert("PYTHONPATH".to_string(), merged);
    }

    if let Some(path) = with_node_bin_path(app_dir) {
        runtime_env.insert("PATH".to_string(), path);
    }

    let uses_python_runtime = command_looks_python_related(&entry_command)
        || install_command
            .as_deref()
            .map(command_looks_python_related)
            .unwrap_or(false);
    if uses_python_runtime {
        match ensure_local_python_venv(app_dir).await {
            Ok((venv_dir, venv_bin_dir)) => {
                if let Some(merged_path) =
                    prepend_path_entry(&venv_bin_dir, runtime_env.get("PATH").map(|v| v.as_str()))
                {
                    runtime_env.insert("PATH".to_string(), merged_path);
                }
                runtime_env.insert(
                    "VIRTUAL_ENV".to_string(),
                    venv_dir.to_string_lossy().to_string(),
                );
            }
            Err(err) => {
                tracing::warn!(
                    "Python venv bootstrap unavailable for app {}. Falling back to system Python: {}",
                    app_id,
                    err
                );
            }
        }
        runtime_env.insert("PIP_DISABLE_PIP_VERSION_CHECK".to_string(), "1".to_string());
        runtime_env.insert("PIP_BREAK_SYSTEM_PACKAGES".to_string(), "1".to_string());
    }

    if let Some(ref cmd) = install_command {
        let output = run_local_command_with_progress(
            cmd,
            "install_command",
            app_dir,
            &runtime_env,
            600,
            &stream_tx,
            "install",
        )
        .await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let detail = if !stderr.trim().is_empty() {
                stderr.trim().to_string()
            } else {
                stdout.trim().to_string()
            };
            anyhow::bail!("install_command failed for app {}: {}", app_id, detail);
        }
    }

    let (stdout_log_path, stderr_log_path) = prepare_local_runtime_log_files(app_dir)?;
    let args = split_command_args(&entry_command, "entry_command")?;
    let (mut child, _resolved_program) = spawn_local_process_with_fallback(
        &args,
        "entry_command",
        app_dir,
        &runtime_env,
        &stdout_log_path,
        &stderr_log_path,
    )
    .await?;

    tokio::time::sleep(std::time::Duration::from_millis(450)).await;
    if let Some(status) = child
        .try_wait()
        .map_err(|e| anyhow::anyhow!("failed to check app {} process status: {}", app_id, e))?
    {
        let log_tail = read_local_runtime_log_tail(app_dir, LOCAL_RUNTIME_LOG_TAIL_BYTES).await;
        if log_tail.is_empty() {
            anyhow::bail!("app {} exited immediately with status {}", app_id, status);
        }
        anyhow::bail!(
            "app {} exited immediately with status {}. Recent runtime logs:\n{}",
            app_id,
            status,
            log_tail
        );
    }

    Ok(child)
}

pub enum DynamicRuntimeHandle {
    Container(String),
    Process(Box<tokio::process::Child>),
}

pub struct DynamicRuntimeLaunch<'a> {
    pub app_id: &'a str,
    pub app_dir: &'a Path,
    pub entry_command: &'a str,
    pub install_command: Option<&'a str>,
    pub port: u16,
    pub extra_env: &'a HashMap<String, String>,
    pub runtime_image: Option<&'a str>,
    pub runtime_preference: RuntimePreference,
    pub stream_tx: Option<Sender<StreamEvent>>,
}

pub async fn launch_dynamic_runtime(
    request: DynamicRuntimeLaunch<'_>,
) -> Result<DynamicRuntimeHandle> {
    let DynamicRuntimeLaunch {
        app_id,
        app_dir,
        entry_command,
        install_command,
        port,
        extra_env,
        runtime_image,
        runtime_preference,
        stream_tx,
    } = request;

    if matches!(runtime_preference, RuntimePreference::Local) && !docker_required() {
        match launch_dynamic_process(
            app_id,
            app_dir,
            entry_command,
            install_command,
            port,
            extra_env,
            stream_tx.clone(),
        )
        .await
        {
            Ok(child) => return Ok(DynamicRuntimeHandle::Process(Box::new(child))),
            Err(local_err) => {
                tracing::warn!(
                    "Local runtime launch unavailable for app {}: {}. Trying container fallback.",
                    app_id,
                    local_err
                );
                match launch_dynamic_container(
                    app_id,
                    app_dir,
                    entry_command,
                    install_command,
                    port,
                    extra_env,
                    runtime_image,
                )
                .await
                {
                    Ok(container_id) => return Ok(DynamicRuntimeHandle::Container(container_id)),
                    Err(container_err) => {
                        return Err(anyhow::anyhow!(
                            "local runtime failed: {} | container fallback failed: {}",
                            local_err,
                            container_err
                        ));
                    }
                }
            }
        }
    }

    match launch_dynamic_container(
        app_id,
        app_dir,
        entry_command,
        install_command,
        port,
        extra_env,
        runtime_image,
    )
    .await
    {
        Ok(container_id) => Ok(DynamicRuntimeHandle::Container(container_id)),
        Err(container_err) => {
            if docker_required() {
                return Err(container_err);
            }
            tracing::warn!(
                "Container launch unavailable for app {}: {}. Falling back to local process runtime.",
                app_id,
                container_err
            );
            match launch_dynamic_process(
                app_id,
                app_dir,
                entry_command,
                install_command,
                port,
                extra_env,
                stream_tx.clone(),
            )
            .await
            {
                Ok(child) => Ok(DynamicRuntimeHandle::Process(Box::new(child))),
                Err(local_err) => Err(anyhow::anyhow!(
                    "container runtime failed: {} | local runtime fallback failed: {}",
                    container_err,
                    local_err
                )),
            }
        }
    }
}

async fn emit_progress(stream_tx: &Option<Sender<StreamEvent>>, message: &str) {
    if let Some(tx) = stream_tx {
        let _ = tx
            .send(StreamEvent::ToolProgress {
                name: "app_deploy".to_string(),
                content: message.to_string(),
                payload: None,
            })
            .await;
    }
}

async fn emit_file_write_progress(
    stream_tx: &Option<Sender<StreamEvent>>,
    filename: &str,
    line: usize,
    total_lines: usize,
    text: &str,
    done: bool,
) {
    if let Some(tx) = stream_tx {
        let status = if total_lines > 0 {
            format!("writing {} line {}/{}", filename, line, total_lines)
        } else {
            format!("writing {} (empty file)", filename)
        };
        let payload = serde_json::json!({
            "kind": "file_write",
            "file": filename,
            "line": line,
            "total_lines": total_lines,
            "text": compact_progress_line(text, 240),
            "done": done,
        });
        let _ = tx
            .send(StreamEvent::ToolProgress {
                name: "app_deploy".to_string(),
                content: status,
                payload: Some(payload),
            })
            .await;
    }
}

async fn write_file_with_progress(
    file_path: &Path,
    filename: &str,
    content: &str,
    stream_tx: &Option<Sender<StreamEvent>>,
) -> Result<()> {
    let mut file = tokio::fs::File::create(file_path).await?;
    if content.is_empty() {
        emit_file_write_progress(stream_tx, filename, 0, 0, "", true).await;
        file.flush().await?;
        return Ok(());
    }

    let segments: Vec<&str> = content.split_inclusive('\n').collect();
    let total_lines = segments.len();
    // Stream normal-sized files line-by-line so the UI can render the code as it is written.
    // Fall back to sampled updates for very large files to avoid flooding the event stream.
    const FILE_WRITE_PROGRESS_LINE_BY_LINE_MAX_LINES: usize = 4_000;
    let emit_every_line = total_lines <= FILE_WRITE_PROGRESS_LINE_BY_LINE_MAX_LINES;
    let sampled_step = (total_lines / 200).clamp(10, 50);
    for (idx, segment) in segments.iter().enumerate() {
        file.write_all(segment.as_bytes()).await?;
        let line_no = idx + 1;
        let is_last = line_no >= total_lines;
        if emit_every_line || line_no == 1 || is_last || (line_no % sampled_step == 0) {
            let line_text = segment.trim_end_matches('\n').trim_end_matches('\r');
            emit_file_write_progress(
                stream_tx,
                filename,
                line_no,
                total_lines,
                line_text,
                is_last,
            )
            .await;
        }
    }
    file.flush().await?;
    Ok(())
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
    /// Whether access guard/key is enforced.
    pub access_guard_enabled: bool,
}

/// Generate a random access key for app authentication
pub fn generate_access_key() -> String {
    format!("ak_{}", uuid::Uuid::new_v4().simple())
}

async fn persist_app_access_guard_meta(
    app_dir: &Path,
    access_guard_enabled: bool,
    access_key: &str,
) -> Result<()> {
    let meta_path = app_dir.join(".app_meta.json");
    let mut meta: serde_json::Value = match tokio::fs::read(&meta_path).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|_| serde_json::json!({})),
        Err(_) => serde_json::json!({}),
    };
    if !meta.is_object() {
        meta = serde_json::json!({});
    }
    meta["access_guard_enabled"] = serde_json::Value::Bool(access_guard_enabled);
    meta["access_key"] = serde_json::Value::String(access_key.to_string());
    let bytes = serde_json::to_vec_pretty(&meta)?;
    tokio::fs::write(&meta_path, bytes).await?;
    Ok(())
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

pub struct DynamicAppRegistration {
    pub title: String,
    pub app_dir: PathBuf,
    pub child: Option<tokio::process::Child>,
    pub container_id: Option<String>,
    pub port: u16,
    pub access_key: String,
    pub access_guard_enabled: bool,
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
            let runtime_mode = if app.is_static {
                "static"
            } else if app.container_id.is_some() {
                "isolated_container"
            } else if app.process.is_some() {
                "local_process_fallback"
            } else {
                "stopped"
            };
            result.push(serde_json::json!({
                "id": id,
                "title": app.title,
                "port": app.port,
                "is_static": app.is_static,
                "running": running,
                "runtime_mode": runtime_mode,
                "is_isolated_runtime": app.container_id.is_some(),
                "created_at": app.created_at.to_rfc3339(),
                "url": format!("/apps/{}/", id),
                "access_url": if app.access_guard_enabled {
                    format!("/apps/{}/?key={}", id, app.access_key)
                } else {
                    format!("/apps/{}/", id)
                },
                "access_key": if app.access_guard_enabled {
                    app.access_key.clone()
                } else {
                    String::new()
                },
                "access_guard_enabled": app.access_guard_enabled,
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

    /// Check runtime liveness for a dynamic app and clear stale runtime handles.
    pub async fn runtime_is_alive(&self, app_id: &str) -> bool {
        let app_handle = {
            let apps = self.apps.read().await;
            apps.get(app_id).cloned()
        };
        let Some(app_handle) = app_handle else {
            return false;
        };
        let mut app = app_handle.write().await;
        if app.is_static {
            return true;
        }

        let mut alive = false;
        if let Some(container_id) = app.container_id.as_ref() {
            alive = is_container_running(container_id).await;
        } else if let Some(child) = app.process.as_mut() {
            alive = matches!(child.try_wait(), Ok(None));
        }

        if !alive {
            app.process = None;
            app.container_id = None;
            app.port = None;
        }
        alive
    }

    /// Register a static app
    pub async fn register_static(
        &self,
        id: String,
        title: String,
        app_dir: PathBuf,
        access_key: String,
        access_guard_enabled: bool,
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
            access_guard_enabled,
        };
        self.apps
            .write()
            .await
            .insert(id, Arc::new(RwLock::new(app)));
    }

    /// Register and start a dynamic app
    pub async fn register_dynamic(&self, id: String, registration: DynamicAppRegistration) {
        let app = RunningApp {
            title: registration.title,
            port: Some(registration.port),
            process: registration.child,
            container_id: registration.container_id,
            app_dir: registration.app_dir,
            is_static: false,
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            request_count: 0,
            access_key: registration.access_key,
            access_guard_enabled: registration.access_guard_enabled,
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
            if !app.access_guard_enabled {
                return true;
            }
            return app.access_key == key;
        }
        false
    }

    /// Whether app requires an access key guard.
    pub async fn access_guard_enabled(&self, app_id: &str) -> bool {
        let app_handle = {
            let apps = self.apps.read().await;
            apps.get(app_id).cloned()
        };
        if let Some(app) = app_handle {
            return app.read().await.access_guard_enabled;
        }
        false
    }

    /// Toggle access guard for an app and optionally rotate its access key.
    pub async fn set_access_guard(
        &self,
        app_id: &str,
        enabled: bool,
        regenerate_key: bool,
    ) -> Result<String> {
        let app_handle = {
            let apps = self.apps.read().await;
            apps.get(app_id).cloned()
        }
        .ok_or_else(|| anyhow::anyhow!("App not found"))?;

        let (app_dir, access_key) = {
            let mut app = app_handle.write().await;
            let should_rotate = regenerate_key || app.access_key.trim().is_empty();
            let next_key = if should_rotate {
                generate_access_key()
            } else {
                app.access_key.clone()
            };
            app.access_guard_enabled = enabled;
            app.access_key = next_key.clone();
            (app.app_dir.clone(), next_key)
        };

        persist_app_access_guard_meta(&app_dir, enabled, &access_key).await?;
        Ok(access_key)
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
                    let runtime_preference = runtime_preference_from_opt(
                        meta.as_ref()
                            .and_then(|m| m.get("runtime_preference").and_then(|c| c.as_str())),
                    );
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

                    // Legacy apps defaulted to guarded mode, so missing flag => true.
                    let access_guard_enabled = meta
                        .as_ref()
                        .and_then(|m| m.get("access_guard_enabled").and_then(|v| v.as_bool()))
                        .unwrap_or(true);
                    // Restore or regenerate access key only when guard is enabled.
                    let access_key = meta
                        .as_ref()
                        .and_then(|m| m.get("access_key").and_then(|k| k.as_str()))
                        .filter(|s| !s.trim().is_empty())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| {
                            if access_guard_enabled {
                                generate_access_key()
                            } else {
                                String::new()
                            }
                        });

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
                                            access_guard_enabled,
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
                                self.register_static(
                                    id.clone(),
                                    title,
                                    path,
                                    access_key.clone(),
                                    access_guard_enabled,
                                )
                                .await;
                                continue;
                            }
                            match launch_dynamic_runtime(DynamicRuntimeLaunch {
                                app_id: &id,
                                app_dir: &path,
                                entry_command: &entry_cmd,
                                install_command: install_command.as_deref(),
                                port,
                                extra_env: &resolved_env,
                                runtime_image: runtime_image.as_deref(),
                                runtime_preference,
                                stream_tx: None,
                            })
                            .await
                            {
                                Ok(runtime_handle) => {
                                    let (container_id, child) = match runtime_handle {
                                        DynamicRuntimeHandle::Container(container_id) => {
                                            (Some(container_id), None)
                                        }
                                        DynamicRuntimeHandle::Process(child) => {
                                            (None, Some(*child))
                                        }
                                    };
                                    self.register_dynamic(
                                        id.clone(),
                                        DynamicAppRegistration {
                                            title,
                                            app_dir: path,
                                            child,
                                            container_id,
                                            port,
                                            access_key: access_key.clone(),
                                            access_guard_enabled,
                                        },
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
                                        access_guard_enabled,
                                    )
                                    .await;
                                }
                            }
                        } else {
                            tracing::warn!("No available port to restart app {}", id);
                            self.register_static(
                                id.clone(),
                                title,
                                path,
                                access_key.clone(),
                                access_guard_enabled,
                            )
                            .await;
                        }
                    } else {
                        // Static app
                        self.register_static(
                            id.clone(),
                            title,
                            path,
                            access_key.clone(),
                            access_guard_enabled,
                        )
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
    if arguments
        .get("repo_url")
        .and_then(|value| value.as_str())
        .is_some_and(|value| !value.trim().is_empty())
    {
        return deploy_repo_bundle(
            config_dir, data_dir, arguments, registry, llm_env, stream_tx,
        )
        .await;
    }

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
    let runtime_preference =
        runtime_preference_from_opt(arguments.get("runtime_preference").and_then(|v| v.as_str()));
    let expose_public = arguments
        .get("expose_public")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let access_guard_enabled = arguments
        .get("access_guard")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let required_inputs = parse_required_inputs(arguments);
    let config_values = parse_config_values(arguments);
    let is_static = entry_command.is_none();

    // Generate app ID and optional access key.
    let app_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let access_key = if access_guard_enabled {
        generate_access_key()
    } else {
        String::new()
    };
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
    )
    .await;
    // Write all files
    let mut written_files = 0usize;
    let mut written_names: Vec<String> = Vec::new();
    for (filename, content) in files {
        let content_str = content.as_str().unwrap_or_default();
        // Prevent path traversal
        if filename.contains("..") || filename.starts_with('/') || filename.starts_with('\\') {
            tracing::warn!("Skipping file with suspicious path: {}", filename);
            continue;
        }
        let file_path = app_dir.join(filename);
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let byte_len = content_str.len();
        write_file_with_progress(&file_path, filename, content_str, &stream_tx)
            .await
            .with_context(|| format!("Failed to write {}", filename))?;
        written_files += 1;
        written_names.push(filename.to_string());
        emit_progress(&stream_tx, &format!("wrote {} ({}B)", filename, byte_len)).await;
    }
    if written_files == 0 {
        anyhow::bail!("No valid files were written. Check filenames and try again.");
    }
    let skipped_files = file_count.saturating_sub(written_files);
    emit_progress(
        &stream_tx,
        &format!(
            "{} / {} files ready (skipped {}): {}",
            written_files,
            file_count,
            skipped_files,
            written_names.join(", ")
        ),
    )
    .await;

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
        "runtime_preference": runtime_preference.as_str(),
        "expose_public": expose_public,
        "repo_url": arguments.get("repo_url").cloned(),
        "repo_ref": arguments.get("repo_ref").cloned(),
        "repo_subdir": arguments.get("repo_subdir").cloned(),
        "repo_bundle_id": arguments.get("repo_bundle_id").cloned(),
        "repo_service_kind": arguments.get("repo_service_kind").cloned(),
        "repo_service_dir": arguments.get("repo_service_dir").cloned(),
        "required_inputs": required_inputs.clone(),
        "required_secrets": required_secret_keys.clone(),
        "required_env": required_secret_keys.clone(),
        "required_config": required_config_keys.clone(),
        "config_values": config_values.clone(),
        "access_guard_enabled": access_guard_enabled,
        "access_key": access_key,
        "created_at": chrono::Utc::now().to_rfc3339(),
    });
    tokio::fs::write(
        app_dir.join(".app_meta.json"),
        serde_json::to_string_pretty(&meta)?,
    )
    .await?;
    emit_progress(&stream_tx, "Saved app metadata").await;

    if is_static {
        // Static app — just register, served directly by HTTP server
        registry
            .register_static(
                app_id.clone(),
                title.to_string(),
                app_dir,
                access_key.clone(),
                access_guard_enabled,
            )
            .await;
        let url = format!("/apps/{}/", app_id);
        tracing::info!("Static app deployed at {}", url);
        emit_progress(&stream_tx, &format!("Static app ready at {}", url)).await;
        return Ok(serde_json::json!({
            "status": "deployed",
            "type": "static",
            "app_id": app_id,
            "url": url,
            "title": title,
            "runtime_preference": runtime_preference.as_str(),
            "expose_public": expose_public,
            "access_key": access_key,
            "access_guard_enabled": access_guard_enabled,
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
    emit_progress(&stream_tx, &format!("Assigned port {}", port)).await;

    if !missing_sensitive.is_empty() || !missing_config.is_empty() {
        let mut missing_all = missing_sensitive.clone();
        for m in &missing_config {
            if !missing_all.iter().any(|x| x == m) {
                missing_all.push(m.clone());
            }
        }
        let llm_reuse_candidates: Vec<String> = missing_sensitive
            .iter()
            .filter(|k| llm_env.get(*k).is_some_and(|v| !v.trim().is_empty()))
            .cloned()
            .collect();
        registry
            .register_static(
                app_id.clone(),
                title.to_string(),
                app_dir,
                access_key.clone(),
                access_guard_enabled,
            )
            .await;
        emit_progress(
            &stream_tx,
            &format!(
                "App created but waiting for required inputs: {}",
                missing_all.join(", ")
            ),
        )
        .await;
        return Ok(serde_json::json!({
            "status": "needs_secrets",
            "type": "dynamic",
            "app_id": app_id,
            "title": title,
            "url": format!("/apps/{}/", app_id),
            "runtime_preference": runtime_preference.as_str(),
            "expose_public": expose_public,
            "access_key": access_key,
            "access_guard_enabled": access_guard_enabled,
            "required_inputs": required_inputs,
            "required_secrets": required_secret_keys.clone(),
            "required_env": required_secret_keys,
            "required_config": required_config_keys,
            "missing_env": missing_sensitive,
            "missing_config": missing_config,
            "llm_reuse_candidates": llm_reuse_candidates,
            "message": "Missing required inputs. For sensitive keys use: set secret KEY=VALUE (or use current llm key for KEY when offered). For non-sensitive values pass config.{KEY} when deploying/restarting."
        })
        .to_string());
    }

    let requirements_path = app_dir.join("requirements.txt");
    let has_requirements = requirements_path.exists()
        && tokio::fs::metadata(&requirements_path)
            .await
            .map(|m| m.len() > 0)
            .unwrap_or(false);
    let has_package_json = app_dir.join("package.json").exists();

    // Each Python app gets its own venv for isolation. Node apps use local node_modules.
    let effective_install_cmd = if let Some(cmd) = install_command {
        Some(cmd.to_string())
    } else if has_requirements {
        Some("python3 -m venv .venv && .venv/bin/pip install -r requirements.txt -q".to_string())
    } else if has_package_json {
        Some("npm install --omit=dev".to_string())
    } else {
        None
    };

    if effective_install_cmd.is_some() {
        emit_progress(&stream_tx, "Installing dependencies...").await;
    } else {
        emit_progress(&stream_tx, "No dependencies to install").await;
    }

    // Start the server process in isolated container
    let entry = entry_command.unwrap();
    tracing::info!(
        "Starting app {} on port {} in isolated runtime",
        app_id,
        port
    );
    emit_progress(&stream_tx, &format!("Starting server on port {}", port)).await;

    let runtime_handle = launch_dynamic_runtime(DynamicRuntimeLaunch {
        app_id: &app_id,
        app_dir: &app_dir,
        entry_command: entry,
        install_command: effective_install_cmd.as_deref(),
        port,
        extra_env: &resolved_env,
        runtime_image: runtime_image.as_deref(),
        runtime_preference,
        stream_tx: stream_tx.clone(),
    })
    .await?;
    let (container_id, child, runtime_label) = match runtime_handle {
        DynamicRuntimeHandle::Container(container_id) => {
            emit_progress(&stream_tx, "Server container started").await;
            (Some(container_id), None, "container")
        }
        DynamicRuntimeHandle::Process(child) => {
            emit_progress(&stream_tx, "Docker unavailable; started local app process").await;
            (None, Some(*child), "local_process")
        }
    };
    let app_dir_for_diagnostics = app_dir.clone();

    registry
        .register_dynamic(
            app_id.clone(),
            DynamicAppRegistration {
                title: title.to_string(),
                app_dir,
                child,
                container_id,
                port,
                access_key: access_key.clone(),
                access_guard_enabled,
            },
        )
        .await;

    // Wait briefly for the server to start
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    if !registry.runtime_is_alive(&app_id).await {
        let log_tail =
            read_local_runtime_log_tail(&app_dir_for_diagnostics, LOCAL_RUNTIME_LOG_TAIL_BYTES)
                .await;
        if log_tail.is_empty() {
            anyhow::bail!("App {} stopped shortly after launch.", app_id);
        }
        anyhow::bail!(
            "App {} stopped shortly after launch. Recent runtime logs:\n{}",
            app_id,
            log_tail
        );
    }

    let url = format!("/apps/{}/", app_id);
    tracing::info!("Dynamic app deployed at {} (port {})", url, port);
    emit_progress(&stream_tx, &format!("Dynamic app ready at {}", url)).await;

    Ok(serde_json::json!({
        "status": "deployed",
        "type": "dynamic",
        "runtime": runtime_label,
        "app_id": app_id,
        "url": url,
        "port": port,
        "title": title,
        "runtime_preference": runtime_preference.as_str(),
        "expose_public": expose_public,
        "access_key": access_key,
        "access_guard_enabled": access_guard_enabled,
    })
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_readme_hints_detects_install_and_start_commands() {
        let readme = r#"
# Demo

```bash
$ npm install
$ npm run dev
```
"#;

        let hints = extract_readme_hints(readme);
        assert_eq!(hints.install_command.as_deref(), Some("npm install"));
        assert_eq!(hints.start_command.as_deref(), Some("npm run dev"));
        assert!(!hints.mentions_compose);
    }

    #[test]
    fn is_allowed_repo_url_rejects_localhost() {
        assert!(is_allowed_repo_url("http://127.0.0.1/repo").is_err());
        assert!(is_allowed_repo_url("http://localhost/repo").is_err());
        assert!(is_allowed_repo_url("https://github.com/openai/demo").is_ok());
    }

    #[test]
    fn plan_repo_services_detects_simple_frontend_and_backend_repo() {
        let repo = tempfile::tempdir().expect("temp repo");
        let frontend = repo.path().join("frontend");
        let backend = repo.path().join("backend");
        std::fs::create_dir_all(&frontend).expect("frontend dir");
        std::fs::create_dir_all(&backend).expect("backend dir");
        std::fs::write(
            repo.path().join("README.md"),
            "# Demo\n\nRun `npm install` then `npm run dev`.\n",
        )
        .expect("readme");
        std::fs::write(
            frontend.join("package.json"),
            r#"{
  "name": "demo-frontend",
  "scripts": { "dev": "vite" },
  "dependencies": { "vite": "^5.0.0", "react": "^18.0.0" }
}"#,
        )
        .expect("frontend manifest");
        std::fs::write(
            frontend.join("index.html"),
            "<!doctype html><html><body>demo</body></html>",
        )
        .expect("frontend html");
        std::fs::write(backend.join("requirements.txt"), "fastapi\nuvicorn\n")
            .expect("backend requirements");
        std::fs::write(
            backend.join("main.py"),
            "from fastapi import FastAPI\napp = FastAPI()\n",
        )
        .expect("backend main");

        let plans = plan_repo_services(repo.path(), "Demo Repo", RepoServiceMode::Auto)
            .expect("repo services");

        assert_eq!(plans.len(), 2);
        assert!(plans.iter().any(|plan| {
            plan.relative_dir == "frontend"
                && plan.kind == RepoServiceKind::Frontend
                && plan
                    .entry_command
                    .as_deref()
                    .is_some_and(|command| command.contains("npm"))
        }));
        assert!(plans.iter().any(|plan| {
            plan.relative_dir == "backend"
                && plan.kind == RepoServiceKind::Backend
                && plan
                    .entry_command
                    .as_deref()
                    .is_some_and(|command| command.contains("uvicorn"))
        }));
    }
}
