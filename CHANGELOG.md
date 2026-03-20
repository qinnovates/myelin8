# Changelog

## 2026-03-19 — Security Hardening

Six patches applied across 4 files. All found via automated red team scan and validated by purple team review. 110 tests passing.

### vault.py

- **Path canonicalization (CWE-22):** Added `Path.resolve()` in `encrypt()` and `decrypt()` before sending paths to the sidecar. Prevents `../` traversal sequences from reaching the Rust binary.

- **Tier allowlist (CWE-20):** Added `_VALID_TIERS` frozenset and validation in `_validate_input()`. Rejects any tier value outside `{hot, warm, cold, frozen, index}`, closing a protocol injection vector via crafted tier strings.

- **Context manager (CWE-404):** Added `__enter__`/`__exit__` to `VaultClient`. Ensures deterministic sidecar process cleanup instead of relying on `__del__`, which CPython does not guarantee to call.

### index_crypto.py

- **Atexit re-lock (CWE-377):** `unlock()` now registers an `atexit` handler that calls `lock()` on process exit. Reduces the plaintext exposure window — if the process exits without explicit `lock()`, the index is re-encrypted automatically. Does not cover `SIGKILL`/OOM (inherent OS limitation).

### pipeline.py

- **Temp file permissions (CWE-732):** Changed all three compression paths (`compress_warm`, `compress_cold`, `compress_frozen` fallback) to call `os.fchmod(fd, 0o600)` on the file descriptor *before* writing content. Previously, `os.chmod()` was called *after* write, leaving a brief window where the file could be readable depending on umask.

### engine.py

- **Recall error path cleanup (CWE-459):** Decrypted intermediate files are now deleted on decompression failure and integrity check failure during `recall()`. Previously, a failed decompression left the decrypted `.zst` file on disk permanently.
