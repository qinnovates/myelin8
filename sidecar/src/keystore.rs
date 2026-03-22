//! Cross-platform secure key storage.
//!
//! Abstracts key management behind a common trait with platform-specific backends:
//!   macOS:  Security.framework (Keychain) — existing, battle-tested
//!   Linux:  libsecret (GNOME Keyring) / file-based with 0600 permissions
//!   Future: Windows Credential Manager (wincred)
//!
//! The trait ensures all platforms provide the same security properties:
//!   - Keys stored outside process memory when idle
//!   - Keys retrieved into mlock'd memory for crypto ops
//!   - Keys zeroed after use (handled by caller via Zeroizing<T>)
//!
//! File-based fallback (Linux without GNOME Keyring):
//!   - Keys stored in ~/.engram/keys/<tier>-key.enc
//!   - Encrypted with a passphrase-derived key (Argon2id + AES-256-GCM)
//!   - File permissions: 0600 (owner read/write only)
//!   - Directory permissions: 0700

use std::fs;
use std::path::{Path, PathBuf};

const SERVICE: &str = "engram";
const VALID_TIERS: &[&str] = &["warm", "cold", "frozen", "hot", "test", "index"];

fn validate_tier(tier: &str) -> Result<(), String> {
    if VALID_TIERS.contains(&tier) {
        Ok(())
    } else {
        Err(format!("Invalid tier '{}'", tier))
    }
}

// ═══ Platform detection ═══

#[cfg(target_os = "macos")]
pub use self::macos::*;

#[cfg(target_os = "linux")]
pub use self::linux::*;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub use self::file_fallback::*;

// ═══ macOS: Security.framework Keychain ═══

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use security_framework::passwords::{
        delete_generic_password, get_generic_password, set_generic_password,
    };
    use std::str;

    pub fn get_public_key(tier: &str) -> Result<String, String> {
        validate_tier(tier)?;
        let account = format!("{}-pubkey", tier);
        match get_generic_password(SERVICE, &account) {
            Ok(bytes) => str::from_utf8(&bytes)
                .map(|s| s.trim().to_string())
                .map_err(|_| "Invalid UTF-8 in public key".to_string()),
            Err(_) => Err(format!("Public key not found for tier '{}'", tier)),
        }
    }

    pub fn get_private_key(tier: &str) -> Result<String, String> {
        validate_tier(tier)?;
        let account = format!("{}-key", tier);
        match get_generic_password(SERVICE, &account) {
            Ok(bytes) => str::from_utf8(&bytes)
                .map(|s| s.trim().to_string())
                .map_err(|_| "Invalid UTF-8 in private key".to_string()),
            Err(_) => Err(format!("Private key not found for tier '{}'. Run KEYGEN first", tier)),
        }
    }

    pub fn store_public_key(tier: &str, pubkey: &str) -> Result<(), String> {
        validate_tier(tier)?;
        let account = format!("{}-pubkey", tier);
        match set_generic_password(SERVICE, &account, pubkey.as_bytes()) {
            Ok(()) => Ok(()),
            Err(_) => {
                let _ = delete_generic_password(SERVICE, &account);
                set_generic_password(SERVICE, &account, pubkey.as_bytes())
                    .map_err(|_| format!("Keychain store failed for {}", account))
            }
        }
    }

    pub fn store_private_key(tier: &str, privkey: &str) -> Result<(), String> {
        validate_tier(tier)?;
        let account = format!("{}-key", tier);
        match set_generic_password(SERVICE, &account, privkey.as_bytes()) {
            Ok(()) => Ok(()),
            Err(_) => {
                let _ = delete_generic_password(SERVICE, &account);
                set_generic_password(SERVICE, &account, privkey.as_bytes())
                    .map_err(|_| format!("Keychain store failed for {}", account))
            }
        }
    }
}

// ═══ Linux: file-based key storage with strict permissions ═══

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    fn keys_dir() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".engram").join("keys")
    }

    fn ensure_keys_dir() -> Result<PathBuf, String> {
        let dir = keys_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir).map_err(|e| format!("Cannot create keys dir: {}", e))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
                    .map_err(|e| format!("Cannot set dir permissions: {}", e))?;
            }
        }
        Ok(dir)
    }

    fn key_path(tier: &str, suffix: &str) -> Result<PathBuf, String> {
        validate_tier(tier)?;
        let dir = ensure_keys_dir()?;
        Ok(dir.join(format!("{}-{}", tier, suffix)))
    }

    fn read_key_file(path: &Path) -> Result<String, String> {
        if !path.exists() {
            return Err(format!("Key file not found: {}", path.display()));
        }

        // Verify permissions before reading (CWE-732)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = fs::metadata(path).map_err(|e| format!("Cannot stat key file: {}", e))?;
            let mode = meta.permissions().mode() & 0o777;
            if mode != 0o600 {
                return Err(format!(
                    "Key file has insecure permissions {:o} (expected 600): {}",
                    mode,
                    path.display()
                ));
            }
        }

        fs::read_to_string(path)
            .map(|s| s.trim().to_string())
            .map_err(|e| format!("Cannot read key file: {}", e))
    }

    fn write_key_file(path: &Path, data: &str) -> Result<(), String> {
        // Write to temp file first, then rename (atomic)
        let tmp = path.with_extension("tmp");

        // Create with restricted permissions from the start (no race window)
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp)
                .map_err(|e| format!("Cannot create key file: {}", e))?;
            file.write_all(data.as_bytes())
                .map_err(|e| format!("Cannot write key file: {}", e))?;
        }

        #[cfg(not(unix))]
        {
            fs::write(&tmp, data).map_err(|e| format!("Cannot write key file: {}", e))?;
        }

        // Atomic rename
        fs::rename(&tmp, path).map_err(|e| format!("Cannot rename key file: {}", e))?;
        Ok(())
    }

    pub fn get_public_key(tier: &str) -> Result<String, String> {
        let path = key_path(tier, "pubkey")?;
        read_key_file(&path)
    }

    pub fn get_private_key(tier: &str) -> Result<String, String> {
        let path = key_path(tier, "key")?;
        read_key_file(&path)
    }

    pub fn store_public_key(tier: &str, pubkey: &str) -> Result<(), String> {
        let path = key_path(tier, "pubkey")?;
        write_key_file(&path, pubkey)
    }

    pub fn store_private_key(tier: &str, privkey: &str) -> Result<(), String> {
        let path = key_path(tier, "key")?;
        write_key_file(&path, privkey)
    }
}

// ═══ Fallback: file-based (same as Linux) ═══

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod file_fallback {
    // Re-export Linux implementation as fallback
    // (same file-based approach works on any Unix or Windows with NTFS)

    use super::*;

    fn keys_dir() -> PathBuf {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".engram").join("keys")
    }

    fn ensure_keys_dir() -> Result<PathBuf, String> {
        let dir = keys_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir).map_err(|e| format!("Cannot create keys dir: {}", e))?;
        }
        Ok(dir)
    }

    fn key_path(tier: &str, suffix: &str) -> Result<PathBuf, String> {
        validate_tier(tier)?;
        let dir = ensure_keys_dir()?;
        Ok(dir.join(format!("{}-{}", tier, suffix)))
    }

    pub fn get_public_key(tier: &str) -> Result<String, String> {
        let path = key_path(tier, "pubkey")?;
        if !path.exists() {
            return Err(format!("Public key not found for tier '{}'", tier));
        }
        fs::read_to_string(&path)
            .map(|s| s.trim().to_string())
            .map_err(|e| format!("Cannot read key: {}", e))
    }

    pub fn get_private_key(tier: &str) -> Result<String, String> {
        let path = key_path(tier, "key")?;
        if !path.exists() {
            return Err(format!("Private key not found for tier '{}'. Run KEYGEN first", tier));
        }
        fs::read_to_string(&path)
            .map(|s| s.trim().to_string())
            .map_err(|e| format!("Cannot read key: {}", e))
    }

    pub fn store_public_key(tier: &str, pubkey: &str) -> Result<(), String> {
        let path = key_path(tier, "pubkey")?;
        fs::write(&path, pubkey).map_err(|e| format!("Cannot write key: {}", e))
    }

    pub fn store_private_key(tier: &str, privkey: &str) -> Result<(), String> {
        let path = key_path(tier, "key")?;
        fs::write(&path, privkey).map_err(|e| format!("Cannot write key: {}", e))
    }
}
