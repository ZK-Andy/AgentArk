use crate::crypto::{KeyManager, KEY_LEN};
use crate::storage::Storage;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

pub const LEGACY_STORAGE_KEYS_KEY: &str = "storage_legacy_keys_v1";
const MAX_LEGACY_STORAGE_KEYS: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ArchivedStorageKey {
    key_b64: String,
    captured_at: String,
}

fn recover_json_vec_from_key_manager<T>(
    key_name: &str,
    raw_value: &[u8],
    candidate: &KeyManager,
    recovery_label: &str,
) -> Option<Vec<T>>
where
    T: DeserializeOwned + Serialize,
{
    let decrypted = match candidate.decrypt(raw_value) {
        Ok(bytes) => bytes,
        Err(_) => return None,
    };

    let parsed = match serde_json::from_slice::<Vec<T>>(&decrypted) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(
                "{} decrypted {}, but JSON parsing still failed: {}",
                recovery_label,
                key_name,
                error
            );
            return None;
        }
    };

    Some(parsed)
}

async fn recover_json_vec_from_legacy_keyfile<T>(
    storage: &Storage,
    config_dir: &Path,
    key: &str,
    raw_value: &[u8],
) -> Option<Vec<T>>
where
    T: DeserializeOwned + Serialize,
{
    let keyfile_path = config_dir.join(".keyfile");
    if !keyfile_path.exists() {
        return None;
    }

    let legacy_key = match KeyManager::load_or_create(&keyfile_path) {
        Ok(key_manager) => key_manager,
        Err(error) => {
            tracing::warn!(
                "Failed to load legacy keyfile while recovering {}: {}",
                key,
                error
            );
            return None;
        }
    };

    let parsed = recover_json_vec_from_key_manager(key, raw_value, &legacy_key, "Legacy keyfile")?;
    if let Ok(json) = serde_json::to_vec(&parsed) {
        match storage.set_encrypted(key, &json).await {
            Ok(()) => {
                tracing::info!(
                    "Recovered encrypted KV payload {} using the legacy keyfile and re-saved it with the active key",
                    key
                );
            }
            Err(error) => {
                tracing::warn!(
                    "Recovered {} with the legacy keyfile but failed to re-save it with the active key: {}",
                    key,
                    error
                );
            }
        }
    }
    Some(parsed)
}

async fn load_archived_storage_keys(storage: &Storage) -> Vec<ArchivedStorageKey> {
    match storage.get_encrypted(LEGACY_STORAGE_KEYS_KEY).await {
        Ok(Some(bytes)) => {
            serde_json::from_slice::<Vec<ArchivedStorageKey>>(&bytes).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

pub async fn load_storage_fallback_key_managers(
    storage: &Storage,
    config_dir: &Path,
) -> Vec<Arc<KeyManager>> {
    let mut managers: Vec<Arc<KeyManager>> = Vec::new();
    let mut seen = std::collections::HashSet::<[u8; KEY_LEN]>::new();

    for entry in load_archived_storage_keys(storage).await {
        let Ok(decoded) = BASE64.decode(entry.key_b64.as_bytes()) else {
            continue;
        };
        if decoded.len() != KEY_LEN {
            continue;
        }
        let mut key_bytes = [0u8; KEY_LEN];
        key_bytes.copy_from_slice(&decoded);
        if seen.insert(key_bytes) {
            managers.push(Arc::new(KeyManager::from_raw_key_bytes(key_bytes)));
        }
    }

    let keyfile_path = config_dir.join(".keyfile");
    if keyfile_path.exists() {
        if let Ok(key_manager) = KeyManager::load_or_create(&keyfile_path) {
            let exported = key_manager.export_key_bytes();
            if seen.insert(exported) {
                managers.push(Arc::new(key_manager));
            }
        }
    }

    managers
}

async fn recover_json_vec_from_archived_storage_keys<T>(
    storage: &Storage,
    key: &str,
    raw_value: &[u8],
) -> Option<Vec<T>>
where
    T: DeserializeOwned + Serialize,
{
    let archived = load_archived_storage_keys(storage).await;
    for entry in archived {
        let Ok(decoded) = BASE64.decode(entry.key_b64.as_bytes()) else {
            continue;
        };
        if decoded.len() != KEY_LEN {
            continue;
        }
        let mut key_bytes = [0u8; KEY_LEN];
        key_bytes.copy_from_slice(&decoded);
        let candidate = KeyManager::from_raw_key_bytes(key_bytes);
        let Some(parsed) = recover_json_vec_from_key_manager(
            key,
            raw_value,
            &candidate,
            "Archived legacy storage key",
        ) else {
            continue;
        };
        if let Ok(json) = serde_json::to_vec(&parsed) {
            match storage.set_encrypted(key, &json).await {
                Ok(()) => {
                    tracing::info!(
                        "Recovered encrypted KV payload {} using an archived legacy storage key and re-saved it with the active key",
                        key
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        "Recovered {} with an archived legacy storage key but failed to re-save it with the active key: {}",
                        key,
                        error
                    );
                }
            }
        }
        return Some(parsed);
    }
    None
}

pub async fn remember_legacy_storage_key(
    storage: &Storage,
    key_manager: &KeyManager,
) -> anyhow::Result<()> {
    let key_b64 = BASE64.encode(key_manager.export_key_bytes());
    let mut archived = load_archived_storage_keys(storage).await;
    if archived.iter().any(|entry| entry.key_b64 == key_b64) {
        return Ok(());
    }
    archived.insert(
        0,
        ArchivedStorageKey {
            key_b64,
            captured_at: chrono::Utc::now().to_rfc3339(),
        },
    );
    archived.truncate(MAX_LEGACY_STORAGE_KEYS);
    storage
        .set_encrypted(LEGACY_STORAGE_KEYS_KEY, &serde_json::to_vec(&archived)?)
        .await
}

pub async fn encrypted_payload_exists(storage: &Storage, key: &str) -> bool {
    matches!(storage.get(key).await, Ok(Some(_)))
}

pub async fn load_json_vec_with_legacy_key_recovery<T>(
    storage: &Storage,
    config_dir: &Path,
    key: &str,
) -> Vec<T>
where
    T: DeserializeOwned + Serialize,
{
    let raw_value = match storage.get(key).await {
        Ok(Some(value)) => value,
        Ok(None) | Err(_) => return Vec::new(),
    };

    if let Ok(parsed) = serde_json::from_slice::<Vec<T>>(&raw_value) {
        return parsed;
    }

    if let Ok(Some(decrypted)) = storage.get_encrypted(key).await {
        if let Ok(parsed) = serde_json::from_slice::<Vec<T>>(&decrypted) {
            return parsed;
        }
    }

    if let Some(parsed) =
        recover_json_vec_from_archived_storage_keys(storage, key, &raw_value).await
    {
        return parsed;
    }

    recover_json_vec_from_legacy_keyfile(storage, config_dir, key, &raw_value)
        .await
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{install_storage_key_manager, Storage};
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct RecoveryEntry {
        message: String,
    }

    #[tokio::test]
    async fn load_json_vec_with_legacy_key_recovery_rehydrates_old_keyfile_payloads() {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = Storage::connect(
            crate::storage::DatabaseConfig::for_tests().expect("test database config"),
        )
        .await
        .unwrap();
        let legacy_key =
            Arc::new(KeyManager::load_or_create(&temp_dir.path().join(".keyfile")).unwrap());
        let active_key = Arc::new(
            KeyManager::from_password("new-password", &[9_u8; crate::crypto::SALT_LEN]).unwrap(),
        );

        install_storage_key_manager(legacy_key.clone());
        let payload = vec![RecoveryEntry {
            message: "old arkpulse history".to_string(),
        }];
        storage
            .set_encrypted("recovery_test_key", &serde_json::to_vec(&payload).unwrap())
            .await
            .unwrap();

        install_storage_key_manager(active_key.clone());
        let recovered = load_json_vec_with_legacy_key_recovery::<RecoveryEntry>(
            &storage,
            temp_dir.path(),
            "recovery_test_key",
        )
        .await;
        assert_eq!(recovered, payload);

        let reencrypted = storage
            .get_encrypted("recovery_test_key")
            .await
            .unwrap()
            .unwrap();
        let reparsed: Vec<RecoveryEntry> = serde_json::from_slice(&reencrypted).unwrap();
        assert_eq!(reparsed, payload);
    }

    #[tokio::test]
    async fn load_json_vec_with_legacy_key_recovery_rehydrates_archived_storage_keys() {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = Storage::connect(
            crate::storage::DatabaseConfig::for_tests().expect("test database config"),
        )
        .await
        .unwrap();
        let old_key = Arc::new(
            KeyManager::from_password("old-password", &[1_u8; crate::crypto::SALT_LEN]).unwrap(),
        );
        let new_key = Arc::new(
            KeyManager::from_password("new-password", &[2_u8; crate::crypto::SALT_LEN]).unwrap(),
        );

        install_storage_key_manager(old_key.clone());
        let payload = vec![RecoveryEntry {
            message: "old arkpulse history".to_string(),
        }];
        storage
            .set_encrypted("recovery_test_key", &serde_json::to_vec(&payload).unwrap())
            .await
            .unwrap();

        install_storage_key_manager(new_key.clone());
        remember_legacy_storage_key(&storage, old_key.as_ref())
            .await
            .unwrap();
        let recovered = load_json_vec_with_legacy_key_recovery::<RecoveryEntry>(
            &storage,
            temp_dir.path(),
            "recovery_test_key",
        )
        .await;
        assert_eq!(recovered, payload);
    }
}
