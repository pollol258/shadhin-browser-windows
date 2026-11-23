use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::Foundation::{LocalFree, HLOCAL};
#[cfg(windows)]
use windows::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordEntry {
    pub id: String,
    pub origin: String,
    pub username: String,
    pub encrypted_password: String, // base64 encoded DPAPI blob
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordEntryView {
    pub id: String,
    pub origin: String,
    pub username: String,
    pub password_preview: String,
    pub created_at: String,
}

pub struct PasswordManager {
    vault_path: PathBuf,
    entries: Arc<Mutex<Vec<PasswordEntry>>>,
}

impl PasswordManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let vault_path = app_data_dir.join("passwords.vault.json");
        let entries = if vault_path.exists() {
            match fs::read_to_string(&vault_path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };

        Self {
            vault_path,
            entries: Arc::new(Mutex::new(entries)),
        }
    }

    #[cfg(windows)]
    pub fn encrypt_dpapi(data: &[u8], description: &str) -> anyhow::Result<Vec<u8>> {
        let desc_utf16: Vec<u16> = description.encode_utf16().chain(std::iter::once(0)).collect();
        let data_in = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut data_out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };

        unsafe {
            CryptProtectData(
                &data_in,
                PCWSTR(desc_utf16.as_ptr()),
                None,
                None,
                None,
                0,
                &mut data_out,
            )?;

            let slice = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize);
            let result = slice.to_vec();
            let _ = LocalFree(HLOCAL(data_out.pbData as _));
            Ok(result)
        }
    }

    #[cfg(not(windows))]
    pub fn encrypt_dpapi(data: &[u8], _description: &str) -> anyhow::Result<Vec<u8>> {
        // Fallback for non-windows compilation
        Ok(data.to_vec())
    }

    #[cfg(windows)]
    pub fn decrypt_dpapi(encrypted_data: &[u8]) -> anyhow::Result<Vec<u8>> {
        let data_in = CRYPT_INTEGER_BLOB {
            cbData: encrypted_data.len() as u32,
            pbData: encrypted_data.as_ptr() as *mut u8,
        };
        let mut data_out = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };

        unsafe {
            CryptUnprotectData(
                &data_in,
                None,
                None,
                None,
                None,
                0,
                &mut data_out,
            )?;

            let slice = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize);
            let result = slice.to_vec();
            let _ = LocalFree(HLOCAL(data_out.pbData as _));
            Ok(result)
        }
    }

    #[cfg(not(windows))]
    pub fn decrypt_dpapi(encrypted_data: &[u8]) -> anyhow::Result<Vec<u8>> {
        Ok(encrypted_data.to_vec())
    }

    pub fn save_password(&self, origin: &str, username: &str, password: &str) -> anyhow::Result<()> {
        let encrypted_bytes = Self::encrypt_dpapi(password.as_bytes(), &format!("Shadhin-Browser: {}", origin))?;
        use base64::Engine;
        let encrypted_b64 = base64::engine::general_purpose::STANDARD.encode(encrypted_bytes);

        let mut entries = self.entries.lock().unwrap();
        // Remove existing entry for same origin + username if exists
        entries.retain(|e| !(e.origin == origin && e.username == username));

        entries.push(PasswordEntry {
            id: Uuid::new_v4().to_string(),
            origin: origin.to_string(),
            username: username.to_string(),
            encrypted_password: encrypted_b64,
            created_at: chrono::Local::now().to_rfc3339(),
        });

        self.persist(&entries)?;
        Ok(())
    }

    pub fn get_credentials_for_origin(&self, origin: &str) -> Option<(String, String)> {
        let entries = self.entries.lock().unwrap();
        for entry in entries.iter() {
            if entry.origin == origin {
                use base64::Engine;
                if let Ok(enc_bytes) = base64::engine::general_purpose::STANDARD.decode(&entry.encrypted_password) {
                    if let Ok(dec_bytes) = Self::decrypt_dpapi(&enc_bytes) {
                        if let Ok(pwd) = String::from_utf8(dec_bytes) {
                            return Some((entry.username.clone(), pwd));
                        }
                    }
                }
            }
        }
        None
    }

    pub fn get_all_entries(&self) -> Vec<PasswordEntryView> {
        let entries = self.entries.lock().unwrap();
        entries
            .iter()
            .map(|e| {
                let mut pwd_clear = "••••••••".to_string();
                use base64::Engine;
                if let Ok(enc_bytes) = base64::engine::general_purpose::STANDARD.decode(&e.encrypted_password) {
                    if let Ok(dec_bytes) = Self::decrypt_dpapi(&enc_bytes) {
                        if let Ok(pwd) = String::from_utf8(dec_bytes) {
                            pwd_clear = pwd;
                        }
                    }
                }
                PasswordEntryView {
                    id: e.id.clone(),
                    origin: e.origin.clone(),
                    username: e.username.clone(),
                    password_preview: pwd_clear,
                    created_at: e.created_at.clone(),
                }
            })
            .collect()
    }

    pub fn delete_password(&self, id: &str) -> anyhow::Result<()> {
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|e| e.id != id);
        self.persist(&entries)?;
        Ok(())
    }

    pub fn clear_all(&self) -> anyhow::Result<()> {
        let mut entries = self.entries.lock().unwrap();
        entries.clear();
        self.persist(&entries)?;
        Ok(())
    }

    fn persist(&self, entries: &[PasswordEntry]) -> anyhow::Result<()> {
        if let Some(parent) = self.vault_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(entries)?;
        fs::write(&self.vault_path, json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpapi_encryption_decryption_roundtrip() {
        let sample_secret = "super_secure_p@ssw0rd_123!_#";
        let description = "Unit Test Secret";

        let encrypted = PasswordManager::encrypt_dpapi(sample_secret.as_bytes(), description)
            .expect("DPAPI encryption must succeed");
        assert!(!encrypted.is_empty());
        assert_ne!(encrypted, sample_secret.as_bytes());

        let decrypted_bytes = PasswordManager::decrypt_dpapi(&encrypted)
            .expect("DPAPI decryption must succeed");
        let decrypted_str = String::from_utf8(decrypted_bytes).expect("Valid UTF-8");

        assert_eq!(decrypted_str, sample_secret);
    }

    #[test]
    fn test_password_manager_isolated_vault() {
        let temp_dir = std::env::temp_dir().join(format!("shadhin_pwd_test_{}", Uuid::new_v4()));
        let _ = fs::create_dir_all(&temp_dir);

        let mgr = PasswordManager::new(temp_dir.clone());
        assert!(mgr.get_all_entries().is_empty());

        // Save password
        mgr.save_password("https://github.com", "pollol", "gh_pass_secret!")
            .expect("save password succeeds");

        let entries = mgr.get_all_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].origin, "https://github.com");
        assert_eq!(entries[0].username, "pollol");
        assert_eq!(entries[0].password_preview, "gh_pass_secret!");

        // Retrieve credential for origin
        let cred = mgr.get_credentials_for_origin("https://github.com");
        assert_eq!(cred, Some(("pollol".to_string(), "gh_pass_secret!".to_string())));

        // Delete password
        let entry_id = entries[0].id.clone();
        mgr.delete_password(&entry_id).expect("delete password succeeds");
        assert!(mgr.get_all_entries().is_empty());

        // Clean up temporary test vault directory
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
