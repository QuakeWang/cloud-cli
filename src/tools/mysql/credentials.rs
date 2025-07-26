use super::client::MySQLTool;
use crate::config_loader::MySQLConfig;
use crate::error::Result;
use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::{Engine as _, engine::general_purpose};
use dialoguer::{Confirm, Input, Password};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

type Aes256GcmKey = aes_gcm::Key<Aes256Gcm>;

const CONFIG_DIR: &str = ".config/cloud-cli";
const KEY_FILE: &str = "key";

#[derive(Debug)]
pub struct MySQLCredentials {
    pub user: String,
    pub password: String,
}

pub struct CredentialManager {
    key: Aes256GcmKey,
}

impl CredentialManager {
    pub fn new() -> Result<Self> {
        let key = Self::load_or_generate_key()?;
        Ok(Self { key })
    }

    pub fn prompt_for_credentials(&self) -> Result<(String, String)> {
        let user: String = Input::new()
            .with_prompt("Enter Doris username")
            .default("root".to_string())
            .interact()?;
        let password = Password::new()
            .with_prompt("Enter Doris password (press Enter for empty)")
            .allow_empty_password(true)
            .interact()?;
        Ok((user, password))
    }

    pub fn prompt_credentials_with_connection_test(&self) -> Result<(String, String)> {
        loop {
            let (user, password) = self.prompt_for_credentials()?;

            let (host, port) = match MySQLTool::get_connection_params() {
                Ok(params) => params,
                Err(e) => {
                    eprintln!(
                        "[!] Warning: Could not get connection parameters, using default values: {}",
                        e
                    );
                    ("127.0.0.1".to_string(), 9030)
                }
            };

            // println!("Testing MySQL connection to {}:{}...", host, port);

            match MySQLTool::test_connection(&host, port, &user, &password) {
                Ok(_) => {
                    println!("✅ Doris connection successful!");
                    return Ok((user, password));
                }
                Err(e) => {
                    eprintln!("❌ {}", e);

                    let retry = Confirm::new()
                        .with_prompt(
                            "Connection failed. Would you like to re-enter the credentials?",
                        )
                        .default(true)
                        .interact()?;

                    if !retry {
                        return Err(e);
                    }
                }
            }
        }
    }

    pub fn encrypt_credentials(&self, user: &str, password: &str) -> Result<MySQLConfig> {
        let encrypted_password = self.encrypt_password(password)?;
        Ok(MySQLConfig {
            user: user.to_string(),
            password: encrypted_password,
        })
    }

    pub fn decrypt_password(&self, encrypted: &str) -> Result<String> {
        if encrypted.is_empty() {
            return Ok(String::new());
        }
        let combined = general_purpose::STANDARD.decode(encrypted).map_err(|e| std::io::Error::other(format!("Base64 decode failed: {e}")))?;
        if combined.len() < 12 {
            return Err(std::io::Error::other("Invalid encrypted data").into());
        }
        let (nonce_bytes, ciphertext) = combined.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let cipher = Aes256Gcm::new(&self.key);
        let plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|e| std::io::Error::other(format!("Decryption failed: {e}")))?;
        let s = String::from_utf8(plaintext)
            .map_err(|e| std::io::Error::other(format!("UTF8 decode failed: {e}")))?;
        Ok(s)
    }

    fn load_or_generate_key() -> Result<Aes256GcmKey> {
        let key_path = Self::get_key_path()?;
        if key_path.exists() {
            let mut buf = [0u8; 32];
            let mut f = fs::File::open(&key_path)?;
            f.read_exact(&mut buf)?;
            Ok(*Key::<Aes256Gcm>::from_slice(&buf))
        } else {
            let mut buf = [0u8; 32];
            OsRng.fill_bytes(&mut buf);
            if let Some(parent) = key_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut f = fs::File::create(&key_path)?;
            f.write_all(&buf)?;
            fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600))?;
            Ok(*Key::<Aes256Gcm>::from_slice(&buf))
        }
    }

    fn get_config_dir() -> Result<PathBuf> {
        dirs::home_dir()
            .map(|home| home.join(CONFIG_DIR))
            .ok_or_else(|| std::io::Error::other("Could not determine home directory").into())
    }

    fn get_key_path() -> Result<PathBuf> {
        Ok(Self::get_config_dir()?.join(KEY_FILE))
    }

    fn encrypt_password(&self, password: &str) -> Result<String> {
        let cipher = Aes256Gcm::new(&self.key);
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, password.as_bytes())
            .map_err(|e| std::io::Error::other(format!("Encryption failed: {e}")))?;
        let mut combined = Vec::new();
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);
        Ok(general_purpose::STANDARD.encode(combined))
    }
}
