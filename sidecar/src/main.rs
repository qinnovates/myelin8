//! engram-vault: Secure crypto sidecar for Engram
//!
//! This binary handles ALL private key operations so that key material
//! NEVER enters Python's address space. It is the security boundary.
//!
//! Architecture:
//!   Python (Engram) → stdin command → engram-vault → age → stdout result
//!
//! Key material flow:
//!   macOS Keychain (software keychain, NOT Secure Enclave) →
//!   this process (mlock'd, zeroize-on-drop) → age stdin pipe
//!
//! Key NEVER: touches disk, enters Python, appears in process args
//! Key MAY: exist in this process's locked memory during decrypt (milliseconds)
//!
//! Security hardening:
//!   - mlockall: prevents key material from being swapped to disk
//!   - RLIMIT_CORE=0: prevents core dumps from leaking key material
//!   - zeroize: deterministic memory zeroing (not GC-dependent)
//!   - env_clear: age subprocess gets clean environment (no LD_PRELOAD)
//!   - Input validation: reject paths with newlines/nulls (injection prevention)
//!
//! Protocol (stdin/stdout, one command per line):
//!   ENCRYPT <input_path> <output_path> <tier> → OK | ERROR <msg>
//!   DECRYPT <input_path> <output_path> <tier> → OK | ERROR <msg>
//!   KEYGEN <tier>                              → OK <pubkey> | ERROR <msg>
//!   PING                                       → PONG
//!   QUIT                                       → BYE

use std::io::{self, BufRead, Write};
use std::process::{Command, Stdio};
use zeroize::Zeroize;

mod keychain;

fn main() {
    // === Security hardening at startup ===

    #[cfg(unix)]
    unsafe {
        // 1. Lock memory — prevent key material from being swapped to disk
        let mlock_result = libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE);
        if mlock_result != 0 {
            eprintln!(
                "WARNING: mlockall failed (errno {}). Key material may be swappable. \
                 Increase ulimit -l or run with appropriate privileges.",
                *libc::__error()
            );
        }

        // 2. Disable core dumps — prevent crash from leaking key material
        let zero_core = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        libc::setrlimit(libc::RLIMIT_CORE, &zero_core);
    }

    // === Main command loop ===

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let parts: Vec<&str> = line.trim().splitn(4, ' ').collect();
        if parts.is_empty() {
            continue;
        }

        let response = match parts[0].to_uppercase().as_str() {
            "PING" => "PONG".to_string(),
            "QUIT" => {
                let _ = writeln!(stdout, "BYE");
                break;
            }
            "ENCRYPT" => {
                if parts.len() < 4 {
                    "ERROR Usage: ENCRYPT <input> <output> <tier>".to_string()
                } else if let Err(e) = validate_path(parts[1])
                    .and(validate_path(parts[2]))
                    .and(validate_tier(parts[3]))
                {
                    e
                } else {
                    handle_encrypt(parts[1], parts[2], parts[3])
                }
            }
            "DECRYPT" => {
                if parts.len() < 4 {
                    "ERROR Usage: DECRYPT <input> <output> <tier>".to_string()
                } else if let Err(e) = validate_path(parts[1])
                    .and(validate_path(parts[2]))
                    .and(validate_tier(parts[3]))
                {
                    e
                } else {
                    handle_decrypt(parts[1], parts[2], parts[3])
                }
            }
            "KEYGEN" => {
                if parts.len() < 2 {
                    "ERROR Usage: KEYGEN <tier>".to_string()
                } else if let Err(e) = validate_tier(parts[1]) {
                    e
                } else {
                    handle_keygen(parts[1])
                }
            }
            _ => "ERROR Unknown command".to_string(),
        };

        // Audit log to stderr (not stdout — stdout is the protocol channel)
        eprintln!(
            "{} {}",
            chrono_now(),
            response.split_whitespace().next().unwrap_or("?")
        );

        let _ = writeln!(stdout, "{}", response);
        let _ = stdout.flush();
    }
}

// === Input validation (prevents command injection via newlines/nulls) ===

fn validate_path(path: &str) -> Result<(), String> {
    if path.contains('\n') || path.contains('\r') || path.contains('\0') {
        Err("ERROR Invalid path — contains newline or null byte".to_string())
    } else if path.is_empty() {
        Err("ERROR Empty path".to_string())
    } else {
        Ok(())
    }
}

fn validate_tier(tier: &str) -> Result<(), String> {
    match tier {
        "warm" | "cold" | "frozen" | "hot" | "test" => Ok(()),
        _ => Err(format!(
            "ERROR Invalid tier '{}' — must be warm, cold, or frozen",
            &tier[..tier.len().min(10)]
        )),
    }
}

fn chrono_now() -> String {
    // Simple timestamp without pulling in chrono crate
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", secs)
}

// === Encrypt (public key only — no secret involved) ===

fn handle_encrypt(input: &str, output: &str, tier: &str) -> String {
    let pubkey = match keychain::get_public_key(tier) {
        Ok(k) => k,
        Err(e) => return format!("ERROR {}", e),
    };

    // Clear environment to prevent LD_PRELOAD / PATH hijacking of age
    let result = Command::new("age")
        .args(["-r", &pubkey, "-o", output, input])
        .env_clear()
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status();

    match result {
        Ok(status) if status.success() => "OK".to_string(),
        Ok(_) => "ERROR age encrypt failed".to_string(),
        Err(e) => format!("ERROR age not found: {}", e),
    }
}

// === Decrypt (private key from Keychain → age stdin → zeroed) ===

fn handle_decrypt(input: &str, output: &str, tier: &str) -> String {
    // Retrieve private key — stays in this process's mlock'd memory only
    let mut private_key = match keychain::get_private_key(tier) {
        Ok(k) => k,
        Err(e) => return format!("ERROR {}", e),
    };

    // Pipe key to age via stdin — clear environment to prevent hijacking
    let mut child = match Command::new("age")
        .args(["-d", "-i", "/dev/stdin", "-o", output, input])
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            private_key.zeroize();
            return format!("ERROR Failed to spawn age: {}", e);
        }
    };

    // Write private key to age's stdin pipe
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(private_key.as_bytes());
        // stdin dropped here — closes the pipe
    }

    // Zero the key IMMEDIATELY after piping
    private_key.zeroize();

    match child.wait() {
        Ok(status) if status.success() => "OK".to_string(),
        Ok(_) => "ERROR age decrypt failed".to_string(),
        Err(e) => format!("ERROR age process error: {}", e),
    }
}

// === Keygen (age-keygen → Keychain, private key zeroed) ===

fn handle_keygen(tier: &str) -> String {
    // Check if keys already exist — refuse to silently overwrite
    if keychain::get_private_key(tier).is_ok() {
        return format!(
            "ERROR Key already exists for tier '{}'. Delete it first or use a different tier.",
            tier
        );
    }

    // Run age-keygen with clean environment
    let output = match Command::new("age-keygen")
        .env_clear()
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
    {
        Ok(o) => o,
        Err(e) => return format!("ERROR age-keygen not found: {}", e),
    };

    if !output.status.success() {
        return "ERROR age-keygen failed".to_string();
    }

    // Parse output — minimize intermediate String copies
    let mut pubkey = String::new();
    let mut privkey_bytes: Vec<u8> = Vec::new();

    for line in output.stdout.split(|&b| b == b'\n') {
        let trimmed = std::str::from_utf8(line).unwrap_or("").trim();
        if trimmed.starts_with("# public key:") {
            pubkey = trimmed
                .split("# public key:")
                .nth(1)
                .unwrap_or("")
                .trim()
                .to_string();
        } else if trimmed.starts_with("AGE-SECRET-KEY-") {
            privkey_bytes = trimmed.as_bytes().to_vec();
        }
    }

    if pubkey.is_empty() || privkey_bytes.is_empty() {
        privkey_bytes.zeroize();
        return "ERROR Failed to parse age-keygen output".to_string();
    }

    // Convert to string for Keychain storage, then zero the vec
    let privkey_str = String::from_utf8_lossy(&privkey_bytes).to_string();
    privkey_bytes.zeroize();

    // Store in Keychain
    if let Err(e) = keychain::store_public_key(tier, &pubkey) {
        let mut pk = privkey_str;
        pk.zeroize();
        return format!("ERROR Failed to store public key: {}", e);
    }

    let store_result = keychain::store_private_key(tier, &privkey_str);

    // Zero private key string
    let mut pk = privkey_str;
    pk.zeroize();

    match store_result {
        Ok(()) => format!("OK {}", pubkey),
        Err(e) => format!("ERROR Failed to store private key: {}", e),
    }
}

// === libc bindings ===

#[cfg(unix)]
mod libc {
    #[repr(C)]
    pub struct rlimit {
        pub rlim_cur: u64,
        pub rlim_max: u64,
    }

    pub const RLIMIT_CORE: i32 = 4; // macOS value
    pub const MCL_CURRENT: i32 = 1;
    pub const MCL_FUTURE: i32 = 2;

    extern "C" {
        pub fn mlockall(flags: i32) -> i32;
        pub fn setrlimit(resource: i32, rlp: *const rlimit) -> i32;
        pub fn __error() -> *mut i32; // macOS errno
    }
}
