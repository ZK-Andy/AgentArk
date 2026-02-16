//! Sandbox implementations for action execution

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::RuntimeConfig;

/// Sandbox execution mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SandboxMode {
    /// No sandbox - run directly on host
    Native,
    /// WASM sandbox - lightweight, fast
    Wasm,
    /// Docker sandbox - full isolation
    Docker,
}

impl Default for SandboxMode {
    fn default() -> Self {
        Self::Wasm
    }
}

/// Action execution sandbox
pub struct ActionSandbox {
    _wasm_engine: wasmtime::Engine,
    _memory_limit: u64,
}

impl ActionSandbox {
    pub fn new(config: &RuntimeConfig) -> Result<Self> {
        let engine = wasmtime::Engine::default();

        Ok(Self {
            _wasm_engine: engine,
            _memory_limit: config.wasm_memory_limit,
        })
    }
}
