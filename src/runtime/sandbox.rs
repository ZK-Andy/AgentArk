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
    #[allow(dead_code)]
    wasm_engine: wasmtime::Engine,
    #[allow(dead_code)]
    memory_limit: u64,
}

impl ActionSandbox {
    pub fn new(config: &RuntimeConfig) -> Result<Self> {
        let mut wasm_config = wasmtime::Config::new();
        wasm_config.wasm_component_model(false); // Use core WASM

        let engine = wasmtime::Engine::new(&wasm_config)?;

        Ok(Self {
            wasm_engine: engine,
            memory_limit: config.wasm_memory_limit,
        })
    }

    /// Execute a WASM module
    #[allow(dead_code)]
    pub async fn execute_wasm(
        &self,
        wasm_bytes: &[u8],
        function: &str,
        input: &[u8],
    ) -> Result<Vec<u8>> {
        let module = wasmtime::Module::new(&self.wasm_engine, wasm_bytes)?;

        let mut store = wasmtime::Store::new(&self.wasm_engine, ());

        let instance = wasmtime::Instance::new(&mut store, &module, &[])?;

        // Get the function
        let func = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, function)?;

        // Allocate input in WASM memory
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| anyhow::anyhow!("No memory export"))?;

        let input_ptr = 0i32; // Simplified - in production, use proper allocation
        memory.write(&mut store, input_ptr as usize, input)?;

        // Call function
        let result_ptr = func.call(&mut store, (input_ptr, input.len() as i32))?;

        // Read result
        let mut result = vec![0u8; 1024]; // Simplified - in production, get actual size
        memory.read(&store, result_ptr as usize, &mut result)?;

        Ok(result)
    }

    /// Create a sandboxed environment for native execution
    #[allow(dead_code)]
    pub fn create_native_sandbox(&self) -> NativeSandbox {
        NativeSandbox::new()
    }
}

/// Native sandbox with restricted capabilities
#[allow(dead_code)]
pub struct NativeSandbox {
    allowed_paths: Vec<std::path::PathBuf>,
    allowed_env: Vec<String>,
}

impl NativeSandbox {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            allowed_paths: vec![],
            allowed_env: vec![],
        }
    }

    #[allow(dead_code)]
    pub fn allow_path(&mut self, path: std::path::PathBuf) {
        self.allowed_paths.push(path);
    }

    #[allow(dead_code)]
    pub fn allow_env(&mut self, var: String) {
        self.allowed_env.push(var);
    }

    #[allow(dead_code)]
    pub fn is_path_allowed(&self, path: &std::path::Path) -> bool {
        if self.allowed_paths.is_empty() {
            return true; // No restrictions
        }
        self.allowed_paths.iter().any(|allowed| path.starts_with(allowed))
    }
}

impl Default for NativeSandbox {
    fn default() -> Self {
        Self::new()
    }
}
