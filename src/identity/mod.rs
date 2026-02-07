//! Decentralized Identity (DID) and Verifiable Credentials
//!
//! Implements W3C DID Core specification with did:key method
//! Based on arXiv:2511.02841 "AI Agents with DIDs and VCs"

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Decentralized Identifier (DID) for the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecentralizedId {
    /// The DID string (e.g., "did:key:z6Mk...")
    pub did: String,
    /// The public key (hex encoded)
    pub public_key: String,
}

/// A Verifiable Credential issued to or by the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiableCredential {
    /// Credential ID
    pub id: String,
    /// Credential type
    pub credential_type: Vec<String>,
    /// Who issued this credential
    pub issuer: String,
    /// When it was issued
    pub issuance_date: DateTime<Utc>,
    /// When it expires (optional)
    pub expiration_date: Option<DateTime<Utc>>,
    /// The subject (usually the agent's DID)
    pub subject: CredentialSubject,
    /// Cryptographic proof
    pub proof: CredentialProof,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialSubject {
    pub id: String,
    pub claims: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialProof {
    pub proof_type: String,
    pub created: DateTime<Utc>,
    pub verification_method: String,
    pub proof_purpose: String,
    pub proof_value: String,
}

/// Manages the agent's identity, keys, and credentials
pub struct IdentityManager {
    /// The agent's DID
    did: DecentralizedId,
    /// Signing key (private)
    signing_key: SigningKey,
    /// Verifiable credentials held by this agent
    #[allow(dead_code)]
    credentials: Vec<VerifiableCredential>,
}

impl IdentityManager {
    /// Load existing identity or create a new one
    pub async fn load_or_create(data_dir: &Path) -> Result<Self> {
        let key_path = data_dir.join("identity.key");
        let creds_path = data_dir.join("credentials.json");

        let signing_key = if key_path.exists() {
            // Load existing key
            let key_bytes = std::fs::read(&key_path)?;
            if key_bytes.len() != 32 {
                return Err(anyhow!("Invalid key file"));
            }
            let mut key_array = [0u8; 32];
            key_array.copy_from_slice(&key_bytes);
            SigningKey::from_bytes(&key_array)
        } else {
            // Generate new key
            let signing_key = SigningKey::generate(&mut OsRng);
            std::fs::write(&key_path, signing_key.to_bytes())?;
            signing_key
        };

        let verifying_key = signing_key.verifying_key();
        let public_key_bytes = verifying_key.to_bytes();

        // Create DID using did:key method
        // Multicodec prefix for Ed25519 public key is 0xed01
        let mut multicodec_key = vec![0xed, 0x01];
        multicodec_key.extend_from_slice(&public_key_bytes);
        let did_string = format!("did:key:z{}", bs58::encode(&multicodec_key).into_string());

        let did = DecentralizedId {
            did: did_string,
            public_key: hex::encode(public_key_bytes),
        };

        // Load credentials if they exist
        let credentials = if creds_path.exists() {
            let content = std::fs::read_to_string(&creds_path)?;
            serde_json::from_str(&content)?
        } else {
            vec![]
        };

        Ok(Self {
            did,
            signing_key,
            credentials,
        })
    }

    /// Get the agent's DID string
    pub fn did(&self) -> &str {
        &self.did.did
    }

    /// Get the signing key for creating proofs
    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }

    /// Get the verifying (public) key
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Sign arbitrary data
    pub fn sign(&self, data: &[u8]) -> Signature {
        self.signing_key.sign(data)
    }

    /// Verify a signature
    #[allow(dead_code)]
    pub fn verify(&self, data: &[u8], signature: &Signature) -> bool {
        self.verifying_key().verify(data, signature).is_ok()
    }

    /// Issue a verifiable credential
    #[allow(dead_code)]
    pub fn issue_credential(
        &self,
        subject_did: &str,
        credential_type: Vec<String>,
        claims: serde_json::Value,
        expiration: Option<DateTime<Utc>>,
    ) -> Result<VerifiableCredential> {
        let id = format!("urn:uuid:{}", uuid::Uuid::new_v4());
        let issuance_date = Utc::now();

        let subject = CredentialSubject {
            id: subject_did.to_string(),
            claims,
        };

        // Create the credential without proof first
        let mut cred_for_signing = serde_json::json!({
            "id": id,
            "type": credential_type,
            "issuer": self.did.did,
            "issuanceDate": issuance_date.to_rfc3339(),
            "credentialSubject": subject,
        });

        if let Some(exp) = expiration {
            cred_for_signing["expirationDate"] = serde_json::Value::String(exp.to_rfc3339());
        }

        // Sign the credential
        let canonical = serde_json::to_string(&cred_for_signing)?;
        let signature = self.sign(canonical.as_bytes());

        let proof = CredentialProof {
            proof_type: "Ed25519Signature2020".to_string(),
            created: Utc::now(),
            verification_method: format!("{}#keys-1", self.did.did),
            proof_purpose: "assertionMethod".to_string(),
            proof_value: base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                signature.to_bytes(),
            ),
        };

        Ok(VerifiableCredential {
            id,
            credential_type,
            issuer: self.did.did.clone(),
            issuance_date,
            expiration_date: expiration,
            subject,
            proof,
        })
    }

    /// Verify a credential's signature
    #[allow(dead_code)]
    pub fn verify_credential(&self, credential: &VerifiableCredential) -> Result<bool> {
        // Reconstruct the signed data
        let mut cred_for_verification = serde_json::json!({
            "id": credential.id,
            "type": credential.credential_type,
            "issuer": credential.issuer,
            "issuanceDate": credential.issuance_date.to_rfc3339(),
            "credentialSubject": credential.subject,
        });

        if let Some(exp) = credential.expiration_date {
            cred_for_verification["expirationDate"] = serde_json::Value::String(exp.to_rfc3339());
        }

        let canonical = serde_json::to_string(&cred_for_verification)?;

        // Decode and verify signature
        let sig_bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &credential.proof.proof_value,
        )?;

        if sig_bytes.len() != 64 {
            return Ok(false);
        }

        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&sig_bytes);
        let signature = Signature::from_bytes(&sig_array);

        Ok(self.verify(canonical.as_bytes(), &signature))
    }

    /// Store a credential
    #[allow(dead_code)]
    pub fn add_credential(&mut self, credential: VerifiableCredential) {
        self.credentials.push(credential);
    }

    /// Get all credentials
    #[allow(dead_code)]
    pub fn credentials(&self) -> &[VerifiableCredential] {
        &self.credentials
    }

    /// Find credentials by type
    #[allow(dead_code)]
    pub fn find_credentials(&self, credential_type: &str) -> Vec<&VerifiableCredential> {
        self.credentials
            .iter()
            .filter(|c| c.credential_type.iter().any(|t| t == credential_type))
            .collect()
    }
}
