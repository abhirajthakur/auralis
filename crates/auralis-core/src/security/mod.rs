//! Secret storage abstraction.
//!
//! API keys never live in settings, localStorage, or logs. The shell wires
//! the OS keyring (`keyring` crate → Windows Credential Manager, macOS
//! Keychain, Linux Secret Service). This module defines the contract plus a
//! `MemorySecretStore` used by tests and as an explicit, warned-about
//! fallback when no OS keyring exists.

use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("secret not found: {0}")]
    NotFound(String),
    #[error("keyring backend failed: {0}")]
    Backend(String),
}

/// Credential slots are `(service, account)` pairs; Auralis uses a single
/// service name and one account per provider.
pub trait SecretStore: Send + Sync {
    fn get(&self, account: &str) -> Result<String, SecretError>;
    fn set(&self, account: &str, value: &str) -> Result<(), SecretError>;
    fn delete(&self, account: &str) -> Result<(), SecretError>;
}

pub const SERVICE: &str = "com.auralis.app";

/// Test/dev backend. Keys live only in process memory and are never
/// persisted — if the shell has to use this in production it must warn the
/// user that keys are not protected by the OS.
#[derive(Default)]
pub struct MemorySecretStore {
    map: Mutex<HashMap<String, String>>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemorySecretStore {
    fn get(&self, account: &str) -> Result<String, SecretError> {
        self.map
            .lock()
            .unwrap()
            .get(account)
            .cloned()
            .ok_or_else(|| SecretError::NotFound(account.to_string()))
    }

    fn set(&self, account: &str, value: &str) -> Result<(), SecretError> {
        self.map
            .lock()
            .unwrap()
            .insert(account.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<(), SecretError> {
        self.map.lock().unwrap().remove(account);
        Ok(())
    }
}

/// Convenience wrapper the shell calls with provider ids from `config::keys`.
pub fn provider_key(store: &dyn SecretStore, provider: &str) -> Result<String, SecretError> {
    store.get(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trip() {
        let store = MemorySecretStore::new();
        assert!(matches!(store.get("groq"), Err(SecretError::NotFound(_))));
        store.set("groq", "gsk_test").unwrap();
        assert_eq!(store.get("groq").unwrap(), "gsk_test");
        store.delete("groq").unwrap();
        assert!(store.get("groq").is_err());
        // delete of a missing key is idempotent
        store.delete("groq").unwrap();
    }

    #[test]
    fn service_name_is_stable() {
        assert_eq!(SERVICE, "com.auralis.app");
    }
}
