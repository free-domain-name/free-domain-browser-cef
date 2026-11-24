// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Free Domain Name contributors

//! Lazily created local chat identity.
//!
//! The 32-byte Ed25519 secret lives in a plain file next to the chat settings
//! (`chat-identity.json`). A browser-only user never gets one because this
//! module is only touched after the first `chat_set_enabled(true)`.
//!
//! OS keychain storage was deliberately dropped: the cross-binary keychain
//! entry triggered passwort/authorization prompts on macOS, and because both
//! builds used the same entry the two browsers ended up sharing one signing
//! identity — one node id — so the relay saw a single user and every
//! cross-browser message was filtered as the user's own.

use std::path::{Path, PathBuf};

use iroh::{EndpointId, SecretKey};

use super::protocol::fingerprint;

const IDENTITY_FILE: &str = "chat-identity.json";

pub struct Identity {
    pub key: SecretKey,
    pub node_id: EndpointId,
    pub fingerprint: String,
}

impl Identity {
    /// Generates a throwaway identity. Used by development/test launches
    /// (env-gated) and as the fallback when the identity file cannot be
    /// written; it is never persisted.
    pub fn generate() -> Self {
        let key = SecretKey::generate();
        let node_id = key.public();
        Self {
            fingerprint: fingerprint(&node_id),
            key,
            node_id,
        }
    }

    /// Loads the identity from `config_dir/chat-identity.json`, creating the
    /// file on first use. The file is chmod 0600 on Unix.
    pub fn load_or_create(config_dir: Option<&Path>) -> Result<Self, IdentityError> {
        let path = identity_file_path(config_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(IdentityError::File)?;
        }
        let (key, created) = match std::fs::read_to_string(&path) {
            Ok(text) => (parse_key_text(&text)?, false),
            Err(_) => (SecretKey::generate(), true),
        };
        if created {
            let payload = serde_json::json!({
                "key_hex": hex_encode(key.to_bytes().as_ref()),
            });
            let body = serde_json::to_string_pretty(&payload).map_err(|error| {
                IdentityError::BadKey(format!("identity serialization failed: {error}"))
            })?;
            std::fs::write(&path, body + "\n").map_err(IdentityError::File)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                    .map_err(IdentityError::File)?;
            }
        }
        let node_id = key.public();
        Ok(Self {
            fingerprint: fingerprint(&node_id),
            key,
            node_id,
        })
    }
}

/// Where the identity file lives for a given config dir; falls back to the
/// same temp dir the chat settings store uses.
pub fn identity_file_path(config_dir: Option<&Path>) -> PathBuf {
    config_dir
        .map(|dir| dir.join(IDENTITY_FILE))
        .unwrap_or_else(|| {
            std::env::temp_dir()
                .join("free-domain-browser-cef-chat")
                .join(IDENTITY_FILE)
        })
}

pub fn delete(config_dir: Option<&Path>) -> Result<(), IdentityError> {
    let path = identity_file_path(config_dir);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn parse_key_text(text: &str) -> Result<SecretKey, IdentityError> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|error| {
        IdentityError::BadKey(format!("identity file is not valid JSON: {error}"))
    })?;
    let hex = value
        .get("key_hex")
        .and_then(|key| key.as_str())
        .ok_or_else(|| IdentityError::BadKey("identity file has no key_hex".to_string()))?;
    let bytes = hex_decode(hex).ok_or_else(|| {
        IdentityError::BadKey("identity file key is not 64 hex chars".to_string())
    })?;
    let raw: [u8; 32] = bytes
        .try_into()
        .map_err(|_| IdentityError::BadKey("identity file key is not 32 bytes".to_string()))?;
    Ok(SecretKey::from_bytes(&raw))
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_decode(hex: &str) -> Option<Vec<u8>> {
    let hex = hex.trim();
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).ok())
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("identity file error: {0}")]
    File(#[from] std::io::Error),
    #[error("invalid chat identity: {0}")]
    BadKey(String),
}
