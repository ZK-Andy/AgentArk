use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalServiceHealth {
    pub service: String,
    pub mode: String,
    pub ok: bool,
    #[serde(default)]
    pub details: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStatusResponse {
    pub service: String,
    pub mode: String,
    pub root_dir: PathBuf,
    #[serde(default)]
    pub token_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobResponse {
    pub path: String,
    pub bytes: usize,
}
