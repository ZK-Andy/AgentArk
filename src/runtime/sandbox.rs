//! Sandbox implementations for action execution

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::RuntimeConfig;

/// Sandbox execution mode
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum SandboxMode {
    /// No sandbox - run directly on host
    Native,
    /// WASM sandbox - lightweight, fast
    #[default]
    Wasm,
    /// Docker sandbox - full isolation
    Docker,
}

/// Action execution sandbox
pub struct ActionSandbox {
    wasm_engine: wasmtime::Engine,
    memory_limit: usize,
}

impl ActionSandbox {
    pub fn new(config: &RuntimeConfig) -> Result<Self> {
        let engine = wasmtime::Engine::default();

        Ok(Self {
            wasm_engine: engine,
            memory_limit: config.wasm_memory_limit as usize,
        })
    }

    pub fn engine(&self) -> &wasmtime::Engine {
        &self.wasm_engine
    }

    pub fn new_store(&self) -> wasmtime::Store<wasmtime::StoreLimits> {
        let limits = wasmtime::StoreLimitsBuilder::new()
            .memory_size(self.memory_limit)
            .build();
        let mut store = wasmtime::Store::new(&self.wasm_engine, limits);
        store.limiter(|limits| limits);
        store
    }
}
