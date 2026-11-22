//! Multiple ML AI Code accounts: metadata in JSON, tokens encrypted with Windows DPAPI.
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub label: String,
    pub server: String,
    pub token_hint: String,
    pub created_at: String,
    #[serde(default)]
    pub last_used: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountsFile {
    pub version: u32,
    #[serde(default)]
    pub active: Option<String>,
    #[serde(default)]
    pub accounts: Vec<Account>,
}

impl AccountsFile {
    pub fn find(&self, id: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }
    pub fn active_account(&self) -> Option<&Account> {
        self.active.as_deref().and_then(|id| self.find(id))
    }
}

pub struct Accounts {
    path: PathBuf,
    keys: PathBuf,
}

fn new_id() -> String {
    let mut bytes = [0u8; 8];
    use rand::RngCore;
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn hint_for(token: &str) -> String {
    let tail: String = token.chars().rev().take(4).collect::<Vec<char>>().into_iter().rev().collect();
    format!("mlaid_…{}", tail)
}

impl Accounts {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let keys = path.with_extension("keys");
        std::fs::create_dir_all(&keys)?;
        Ok(Accounts { path: path.to_path_buf(), keys })
    }

    pub fn file(&self) -> AccountsFile {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| AccountsFile { version: 1, active: None, accounts: Vec::new() }),
            Err(_) => AccountsFile { version: 1, active: None, accounts: Vec::new() },
        }
    }

    fn save(&self, file: &AccountsFile) -> Result<()> {
        let text = serde_json::to_string_pretty(file)? + "\n";
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, &self.path)?;
        Ok(())
    }

    fn key_path(&self, id: &str) -> PathBuf {
        self.keys.join(format!("{}.bin", id))
    }

    pub fn token(&self, id: &str) -> Result<String> {
        let raw = std::fs::read(self.key_path(id)).context("no stored token for this account")?;
        let plain = unprotect(&raw)?;
        String::from_utf8(plain).context("stored token is not valid UTF-8")
    }

    /// Adds an account and stores its token encrypted; returns the account.
    pub fn add(&self, label: &str, server: &str, token: &str) -> Result<Account> {
        if token.trim().is_empty() {
            return Err(anyhow!("token is empty"));
        }
        let mut file = self.file();
        let id = new_id();
        let protected = protect(token.as_bytes())?;
        std::fs::write(self.key_path(&id), protected)?;
        let account = Account {
            id: id.clone(),
            label: label.to_string(),
            server: server.to_string(),
            token_hint: hint_for(token),
            created_at: chrono::Utc::now().to_rfc3339(),
            last_used: None,
        };
        file.accounts.push(account.clone());
        if file.active.is_none() {
            file.active = Some(id);
        }
        self.save(&file)?;
        Ok(account)
    }

    pub fn activate(&self, id: &str) -> Result<()> {
        let mut file = self.file();
        if file.find(id).is_none() {
            return Err(anyhow!("no such account"));
        }
        file.active = Some(id.to_string());
        if let Some(account) = file.accounts.iter_mut().find(|a| a.id == id) {
            account.last_used = Some(chrono::Utc::now().to_rfc3339());
        }
        self.save(&file)
    }

    pub fn remove(&self, id: &str) -> Result<()> {
        let mut file = self.file();
        file.accounts.retain(|a| a.id != id);
        if file.active.as_deref() == Some(id) {
            file.active = file.accounts.first().map(|a| a.id.clone());
        }
        self.save(&file)?;
        let _ = std::fs::remove_file(self.key_path(id));
        Ok(())
    }

    /// The credential the proxy should use: the active account, else the CLI credential on disk.
    pub fn active_credential(&self, fallback: &crate::auth::Credential) -> Result<(crate::auth::Credential, String, String)> {
        let file = self.file();
        let Some(account) = file.active_account() else {
            return Ok((fallback.clone(), "unattributed".to_string(), "Unattributed".to_string()));
        };
        let token = self.token(&account.id)?;
        Ok((
            crate::auth::Credential {
                token,
                server: account.server.clone(),
                store: "account".into(),
                label: Some(account.label.clone()),
                expires: None,
            },
            account.id.clone(),
            account.label.clone(),
        ))
    }
}

#[cfg(windows)]
mod dpapi {
    use anyhow::{anyhow, Result};
    use windows_sys::Win32::Foundation::{LocalFree, HLOCAL};
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN,
    };

    fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 }
    }

    unsafe fn take_blob(mut out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        LocalFree(out.pbData as HLOCAL);
        out.pbData = std::ptr::null_mut();
        bytes
    }

    pub fn protect(data: &[u8]) -> Result<Vec<u8>> {
        unsafe {
            let mut input = blob(data);
            let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
            let ok = CryptProtectData(
                &mut input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            );
            if ok == 0 {
                return Err(anyhow!("DPAPI protect failed: {}", std::io::Error::last_os_error()));
            }
            Ok(take_blob(output))
        }
    }

    pub fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
        unsafe {
            let mut input = blob(data);
            let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
            let ok = CryptUnprotectData(
                &mut input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            );
            if ok == 0 {
                return Err(anyhow!("DPAPI unprotect failed: {}", std::io::Error::last_os_error()));
            }
            Ok(take_blob(output))
        }
    }
}

#[cfg(windows)]
fn protect(data: &[u8]) -> Result<Vec<u8>> {
    dpapi::protect(data)
}
#[cfg(windows)]
fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    dpapi::unprotect(data)
}

#[cfg(not(windows))]
fn protect(_data: &[u8]) -> Result<Vec<u8>> {
    Err(anyhow!("account token encryption is only implemented on Windows"))
}
#[cfg(not(windows))]
fn unprotect(_data: &[u8]) -> Result<Vec<u8>> {
    Err(anyhow!("account token decryption is only implemented on Windows"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protect_roundtrip() {
        let secret = b"mlaid_test_token_123";
        let sealed = protect(secret).expect("protect");
        assert_ne!(sealed, secret, "ciphertext must differ from plaintext");
        let opened = unprotect(&sealed).expect("unprotect");
        assert_eq!(opened, secret);
    }

    #[test]
    fn add_activate_remove() {
        let dir = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!("accounts-test-{}", new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Accounts::open(&dir.join("accounts.json")).unwrap();
        let first = store.add("First", "https://code.ml.ai", "mlaid_one").unwrap();
        let second = store.add("Second", "https://code.ml.ai", "mlaid_two").unwrap();
        assert_eq!(store.file().active.as_deref(), Some(first.id.as_str()));
        store.activate(&second.id).unwrap();
        assert_eq!(store.file().active.as_deref(), Some(second.id.as_str()));
        assert_eq!(store.token(&second.id).unwrap(), "mlaid_two");
        let on_disk = std::fs::read(store.key_path(&second.id)).unwrap();
        assert!(!String::from_utf8_lossy(&on_disk).contains("mlaid_two"), "token must not be plaintext on disk");
        store.remove(&second.id).unwrap();
        assert_eq!(store.file().active.as_deref(), Some(first.id.as_str()));
        std::fs::remove_dir_all(&dir).ok();
    }
}
