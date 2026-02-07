//! Cryptographic Execution Proofs (SPEX-inspired)
//!
//! Based on arXiv:2503.18899 "Statistical Proof of Execution"
//! and arXiv:2512.17538 "Binding Agent ID"
//!
//! Every agent action generates a cryptographic proof that can be verified
//! to prove the agent actually performed the claimed action.

use anyhow::Result;
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use uuid::Uuid;

use crate::core::ToolCall;

/// An execution proof for a single agent action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionProof {
    /// Unique proof ID
    pub id: Uuid,

    /// Hash of the action performed (hex encoded)
    pub action_hash: String,

    /// Hash of the input (hex encoded)
    pub input_hash: String,

    /// Hash of the output (hex encoded)
    pub output_hash: String,

    /// Hash of the previous proof (hex encoded)
    pub prev_hash: Option<String>,

    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// Agent's DID
    pub agent_did: String,

    /// Cryptographic signature (hex encoded)
    pub signature: String,
}

impl ExecutionProof {
    /// Compute the hash of this proof (for chaining)
    pub fn hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.id.as_bytes());
        hasher.update(&self.action_hash);
        hasher.update(&self.input_hash);
        hasher.update(&self.output_hash);
        if let Some(prev) = &self.prev_hash {
            hasher.update(prev);
        }
        hasher.update(self.timestamp.timestamp().to_le_bytes());
        hasher.update(self.agent_did.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Verify the proof signature
    #[allow(dead_code)]
    pub fn verify(&self, verifying_key: &ed25519_dalek::VerifyingKey) -> bool {
        let data_to_verify = self.data_for_signing();
        let sig_bytes = match hex::decode(&self.signature) {
            Ok(b) if b.len() == 64 => b,
            _ => return false,
        };
        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&sig_bytes);
        let signature = Signature::from_bytes(&sig_array);
        verifying_key.verify_strict(&data_to_verify, &signature).is_ok()
    }

    fn data_for_signing(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(self.id.as_bytes());
        data.extend_from_slice(self.action_hash.as_bytes());
        data.extend_from_slice(self.input_hash.as_bytes());
        data.extend_from_slice(self.output_hash.as_bytes());
        if let Some(prev) = &self.prev_hash {
            data.extend_from_slice(prev.as_bytes());
        }
        data.extend_from_slice(&self.timestamp.timestamp().to_le_bytes());
        data.extend_from_slice(self.agent_did.as_bytes());
        data
    }
}

/// A verifiable execution trace (chain of proofs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// All proofs in order
    pub proofs: Vec<ExecutionProof>,

    /// Root hash (hash of first proof)
    pub root_hash: Option<String>,

    /// Latest hash
    pub latest_hash: Option<String>,
}

impl ExecutionTrace {
    pub fn new() -> Self {
        Self {
            proofs: Vec::new(),
            root_hash: None,
            latest_hash: None,
        }
    }

    /// Verify the entire chain
    #[allow(dead_code)]
    pub fn verify_chain(&self, verifying_key: &ed25519_dalek::VerifyingKey) -> bool {
        if self.proofs.is_empty() {
            return true;
        }

        let mut prev_hash: Option<String> = None;

        for proof in &self.proofs {
            // Verify signature
            if !proof.verify(verifying_key) {
                return false;
            }

            // Verify chain linkage
            if proof.prev_hash != prev_hash {
                return false;
            }

            prev_hash = Some(proof.hash());
        }

        true
    }

    /// Export trace for external verification
    #[allow(dead_code)]
    pub fn export(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

impl Default for ExecutionTrace {
    fn default() -> Self {
        Self::new()
    }
}

/// Engine for generating and verifying execution proofs
pub struct ProofEngine {
    /// Signing key
    signing_key: SigningKey,

    /// Agent's DID
    agent_did: String,

    /// Current execution trace
    trace: ExecutionTrace,

    /// Storage path
    data_dir: std::path::PathBuf,
}

impl ProofEngine {
    pub fn new(data_dir: &Path, signing_key: &SigningKey) -> Result<Self> {
        let verifying_key = signing_key.verifying_key();
        let public_key_bytes = verifying_key.to_bytes();

        // Reconstruct DID
        let mut multicodec_key = vec![0xed, 0x01];
        multicodec_key.extend_from_slice(&public_key_bytes);
        let agent_did = format!("did:key:z{}", bs58::encode(&multicodec_key).into_string());

        // Load existing trace if present
        let trace_path = data_dir.join("execution_trace.json");
        let trace = if trace_path.exists() {
            let content = std::fs::read_to_string(&trace_path)?;
            serde_json::from_str(&content)?
        } else {
            ExecutionTrace::new()
        };

        Ok(Self {
            signing_key: signing_key.clone(),
            agent_did,
            trace,
            data_dir: data_dir.to_path_buf(),
        })
    }

    /// Generate a proof for an execution
    pub fn generate_proof(
        &mut self,
        input: &str,
        output: &str,
        tool_calls: &[ToolCall],
    ) -> Result<ExecutionProof> {
        let id = Uuid::new_v4();
        let timestamp = Utc::now();

        // Hash the action (tool calls)
        let action_hash = Self::hash_data(&serde_json::to_vec(tool_calls)?);

        // Hash input
        let input_hash = Self::hash_data(input.as_bytes());

        // Hash output
        let output_hash = Self::hash_data(output.as_bytes());

        // Get previous hash for chaining
        let prev_hash = self.trace.latest_hash.clone();

        // Create unsigned proof
        let mut proof = ExecutionProof {
            id,
            action_hash,
            input_hash,
            output_hash,
            prev_hash,
            timestamp,
            agent_did: self.agent_did.clone(),
            signature: String::new(),
        };

        // Sign the proof
        let data = proof.data_for_signing();
        let signature = self.signing_key.sign(&data);
        proof.signature = hex::encode(signature.to_bytes());

        // Update trace
        let proof_hash = proof.hash();
        if self.trace.root_hash.is_none() {
            self.trace.root_hash = Some(proof_hash.clone());
        }
        self.trace.latest_hash = Some(proof_hash);
        self.trace.proofs.push(proof.clone());

        // Persist trace
        self.save_trace()?;

        Ok(proof)
    }

    /// Hash arbitrary data
    fn hash_data(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    }

    /// Save trace to disk
    fn save_trace(&self) -> Result<()> {
        let trace_path = self.data_dir.join("execution_trace.json");
        let content = serde_json::to_string_pretty(&self.trace)?;
        std::fs::write(trace_path, content)?;
        Ok(())
    }

    /// Get the current execution trace
    pub fn trace(&self) -> &ExecutionTrace {
        &self.trace
    }

    /// Export trace for external verification
    #[allow(dead_code)]
    pub fn export_trace(&self) -> Result<String> {
        self.trace.export()
    }

    /// Verify a proof
    #[allow(dead_code)]
    pub fn verify_proof(&self, proof: &ExecutionProof) -> bool {
        let verifying_key = self.signing_key.verifying_key();
        proof.verify(&verifying_key)
    }
}

/// Compact proof receipt for sharing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofReceipt {
    pub proof_id: Uuid,
    pub action_summary: String,
    pub timestamp: DateTime<Utc>,
    pub agent_did: String,
    pub proof_hash: String,
    pub signature: String,
}

impl From<&ExecutionProof> for ProofReceipt {
    fn from(proof: &ExecutionProof) -> Self {
        Self {
            proof_id: proof.id,
            action_summary: format!("Action hash: {}", hex::encode(&proof.action_hash[..8])),
            timestamp: proof.timestamp,
            agent_did: proof.agent_did.clone(),
            proof_hash: hex::encode(proof.hash()),
            signature: hex::encode(&proof.signature[..32]), // Truncated for display
        }
    }
}
