use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use pbkdf2::pbkdf2_hmac;
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const PBKDF2_ITERS: u32 = 100_000;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

#[derive(Clone)]
pub enum SecretBackend {
    Keyring,
    EncryptedFile,
    Unavailable,
}

pub struct SecretStore {
    service_name: String,
    fallback_path: PathBuf,
    backend: SecretBackend,
}

#[derive(Serialize, Deserialize)]
struct EncryptedBlob {
    salt_b64: String,
    nonce_b64: String,
    ciphertext_b64: String,
}

impl SecretStore {
    pub fn new(service_name: &str) -> Self {
        let fallback_path = Self::default_fallback_path(service_name);
        let backend = if Self::keyring_usable(service_name) {
            SecretBackend::Keyring
        } else if std::env::var("AUVRO_FALLBACK_PASSPHRASE").is_ok() {
            SecretBackend::EncryptedFile
        } else {
            SecretBackend::Unavailable
        };

        Self {
            service_name: service_name.to_owned(),
            fallback_path,
            backend,
        }
    }

    pub fn get(&self, key: &str) -> Result<String, String> {
        match self.backend {
            SecretBackend::Keyring => self.get_from_keyring(key),
            SecretBackend::EncryptedFile => self.get_from_fallback(key),
            SecretBackend::Unavailable => Err(
                "No secure secret backend is available (keychain failed and AUVRO_FALLBACK_PASSPHRASE is not set)."
                    .to_owned(),
            ),
        }
    }

    pub fn set(&self, key: &str, value: &str) -> Result<(), String> {
        match self.backend {
            SecretBackend::Keyring => self.set_in_keyring(key, value),
            SecretBackend::EncryptedFile => self.set_in_fallback(key, value),
            SecretBackend::Unavailable => Err(
                "No secure secret backend is available (keychain failed and AUVRO_FALLBACK_PASSPHRASE is not set)."
                    .to_owned(),
            ),
        }
    }

    pub fn delete(&self, key: &str) -> Result<(), String> {
        match self.backend {
            SecretBackend::Keyring => self.delete_from_keyring(key),
            SecretBackend::EncryptedFile => self.delete_from_fallback(key),
            SecretBackend::Unavailable => Err(
                "No secure secret backend is available (keychain failed and AUVRO_FALLBACK_PASSPHRASE is not set)."
                    .to_owned(),
            ),
        }
    }

    fn keyring_usable(service_name: &str) -> bool {
        #[cfg(target_os = "linux")]
        {
            let has_session_bus = std::env::var("DBUS_SESSION_BUS_ADDRESS")
                .ok()
                .is_some_and(|v| !v.trim().is_empty());
            if !has_session_bus {
                return false;
            }
        }

        let Ok(entry) = keyring::Entry::new(service_name, "__keyring_probe__") else {
            return false;
        };

        match entry.get_password() {
            Ok(_) => true,
            Err(keyring::Error::NoEntry) => true,
            Err(_) => false,
        }
    }

    fn get_from_keyring(&self, key: &str) -> Result<String, String> {
        let entry = keyring::Entry::new(&self.service_name, key).map_err(|e| e.to_string())?;
        entry.get_password().map_err(|e| e.to_string())
    }

    fn set_in_keyring(&self, key: &str, value: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(&self.service_name, key).map_err(|e| e.to_string())?;
        entry.set_password(value).map_err(|e| e.to_string())
    }

    fn delete_from_keyring(&self, key: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(&self.service_name, key).map_err(|e| e.to_string())?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(_) => Ok(()),
        }
    }

    fn get_from_fallback(&self, key: &str) -> Result<String, String> {
        let content = fs::read_to_string(&self.fallback_path).map_err(|e| e.to_string())?;
        let blob: EncryptedBlob = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        let map = self.decrypt_map(&blob)?;
        map.get(key)
            .cloned()
            .ok_or_else(|| format!("Secret not found for key: {key}"))
    }

    fn set_in_fallback(&self, key: &str, value: &str) -> Result<(), String> {
        let mut map = if self.fallback_path.exists() {
            let content = fs::read_to_string(&self.fallback_path).map_err(|e| e.to_string())?;
            let blob: EncryptedBlob = serde_json::from_str(&content).map_err(|e| e.to_string())?;
            self.decrypt_map(&blob)?
        } else {
            HashMap::new()
        };

        map.insert(key.to_owned(), value.to_owned());
        let blob = self.encrypt_map(&map)?;

        if let Some(parent) = self.fallback_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let json = serde_json::to_string_pretty(&blob).map_err(|e| e.to_string())?;
        fs::write(&self.fallback_path, json).map_err(|e| e.to_string())
    }

    fn delete_from_fallback(&self, key: &str) -> Result<(), String> {
        if !self.fallback_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&self.fallback_path).map_err(|e| e.to_string())?;
        let blob: EncryptedBlob = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        let mut map = self.decrypt_map(&blob)?;

        map.remove(key);

        let new_blob = self.encrypt_map(&map)?;
        let json = serde_json::to_string_pretty(&new_blob).map_err(|e| e.to_string())?;
        fs::write(&self.fallback_path, json).map_err(|e| e.to_string())
    }

    fn passphrase(&self) -> Result<String, String> {
        std::env::var("AUVRO_FALLBACK_PASSPHRASE")
            .map_err(|_| "AUVRO_FALLBACK_PASSPHRASE is required for AES fallback.".to_owned())
    }

    fn derive_key(&self, salt: &[u8]) -> Result<[u8; 32], String> {
        let mut key = [0u8; 32];
        let passphrase = self.passphrase()?;
        pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), salt, PBKDF2_ITERS, &mut key);
        Ok(key)
    }

    fn encrypt_map(&self, map: &HashMap<String, String>) -> Result<EncryptedBlob, String> {
        let plaintext = serde_json::to_vec(map).map_err(|e| e.to_string())?;

        let mut salt = [0u8; SALT_LEN];
        OsRng.fill_bytes(&mut salt);

        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);

        let key = self.derive_key(&salt)?;
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_ref())
            .map_err(|e| e.to_string())?;

        Ok(EncryptedBlob {
            salt_b64: base64::engine::general_purpose::STANDARD.encode(salt),
            nonce_b64: base64::engine::general_purpose::STANDARD.encode(nonce_bytes),
            ciphertext_b64: base64::engine::general_purpose::STANDARD.encode(ciphertext),
        })
    }

    fn decrypt_map(&self, blob: &EncryptedBlob) -> Result<HashMap<String, String>, String> {
        let salt = base64::engine::general_purpose::STANDARD
            .decode(&blob.salt_b64)
            .map_err(|e| e.to_string())?;
        let nonce_bytes = base64::engine::general_purpose::STANDARD
            .decode(&blob.nonce_b64)
            .map_err(|e| e.to_string())?;
        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(&blob.ciphertext_b64)
            .map_err(|e| e.to_string())?;

        if nonce_bytes.len() != NONCE_LEN {
            return Err("Invalid nonce length in encrypted secret file".to_owned());
        }

        let key = self.derive_key(&salt)?;
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| e.to_string())?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| e.to_string())?;

        serde_json::from_slice(&plaintext).map_err(|e| e.to_string())
    }

    fn default_fallback_path(service_name: &str) -> PathBuf {
        let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join(service_name.to_lowercase())
            .join("secure-secrets.json")
    }
}
