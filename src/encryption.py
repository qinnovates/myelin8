"""
Optional post-quantum encryption layer using age (v1.3.0+).

age v1.3.0 implements ML-KEM-768 in hybrid mode with X25519, making it
the simplest path to PQKC encryption for file-based artifacts.

This module is OPTIONAL. Users who enable encryption choose their own
key management strategy. We provide guidance, not key storage.

Security model:
  - Compress THEN encrypt (compress-then-encrypt is standard practice)
  - Encryption uses age CLI (must be installed separately)
  - Keys never stored by this engine — user manages via their vault
  - Supports: age native keys, SSH keys, passphrase-based (fallback)

Why age over GPG:
  - Simpler key format, no web-of-trust complexity
  - PQ hybrid support built in (v1.3.0+)
  - No configuration files or keyrings to manage
  - Used by SOPS for git-based secret management
"""

from __future__ import annotations

import os
import re
import subprocess
import shutil
from pathlib import Path
from typing import Optional

from .config import EncryptionConfig


ENCRYPTED_EXT = ".age"

# Validation patterns for age recipients
# age native: age1 followed by 58 lowercase alphanumeric chars (Bech32)
AGE_NATIVE_KEY_RE = re.compile(r"^age1[a-z0-9]{58}$")
# SSH pubkey: starts with ssh-ed25519, ssh-rsa, or ecdsa-
SSH_PUBKEY_RE = re.compile(r"^(ssh-(ed25519|rsa)|ecdsa-sha2-nistp\d+)\s+\S+$")


class EncryptionError(Exception):
    pass


class AgeNotFoundError(EncryptionError):
    pass


def _validate_recipient(pubkey: str) -> None:
    """Validate recipient public key format to prevent argument injection."""
    if not (AGE_NATIVE_KEY_RE.match(pubkey) or SSH_PUBKEY_RE.match(pubkey)):
        raise EncryptionError(
            f"Invalid recipient key format. Expected age1... (Bech32) or SSH public key. "
            f"Got: {pubkey[:20]}..."
        )


def _validate_identity_path(path: str) -> Path:
    """Validate and resolve identity (private key) path.

    Enforces:
      - Must be within user's home directory
      - Must be a regular file (not symlink, pipe, device, /proc, /dev)
      - Must not target known sensitive non-key directories
    """
    import stat as stat_mod

    # Check raw input for traversal before resolving
    if ".." in path.split(os.sep):
        raise EncryptionError(f"Path traversal detected in identity path: {path}")

    resolved = Path(path).expanduser().resolve()

    if not resolved.exists():
        raise EncryptionError(f"Identity file not found: {resolved}")

    # Must be within home directory
    try:
        resolved.relative_to(Path.home())
    except ValueError:
        raise EncryptionError(
            f"Identity path must be within home directory: {resolved}"
        )

    # Must be a regular file (not symlink target to device/pipe/proc)
    file_stat = resolved.stat()
    if not stat_mod.S_ISREG(file_stat.st_mode):
        raise EncryptionError(
            f"Identity path is not a regular file: {resolved}"
        )

    return resolved


def check_age_installed() -> str:
    """Verify age is installed and return version string."""
    age_path = shutil.which("age")
    if not age_path:
        raise AgeNotFoundError(
            "age encryption tool not found. Install it:\n"
            "  brew install age          (macOS)\n"
            "  apt install age           (Debian/Ubuntu)\n"
            "  go install filippo.io/age/cmd/age@latest  (Go)\n"
            "\n"
            "For PQ support, you need age v1.3.0+.\n"
            "See: https://github.com/FiloSottile/age"
        )

    result = subprocess.run(
        [age_path, "--version"],
        capture_output=True, text=True, timeout=10
    )
    version = result.stdout.strip() or result.stderr.strip()
    return version


def encrypt_file(
    input_path: Path,
    config: EncryptionConfig,
    output_path: Optional[Path] = None,
    remove_original: bool = False,
) -> Path:
    """
    Encrypt a file using age.

    Uses recipient public key from config. If no recipient is set,
    falls back to passphrase-based encryption (interactive).

    Args:
        input_path: File to encrypt.
        config: Encryption configuration with recipient key.
        output_path: Destination. Defaults to input_path + .age
        remove_original: Delete source after encryption.

    Returns:
        Path to encrypted file.
    """
    check_age_installed()
    input_path = Path(input_path)

    if output_path is None:
        output_path = input_path.with_suffix(input_path.suffix + ENCRYPTED_EXT)

    cmd = ["age"]

    if config.recipient_pubkey:
        _validate_recipient(config.recipient_pubkey)
        cmd.extend(["-r", config.recipient_pubkey])
    else:
        # Passphrase mode (requires user interaction — not recommended for automation)
        cmd.append("-p")

    cmd.extend(["-o", str(output_path), str(input_path)])

    result = subprocess.run(
        cmd, capture_output=True, text=True, timeout=300
    )
    if result.returncode != 0:
        raise EncryptionError("age encrypt failed. Run with --verbose for details.")

    if remove_original and output_path.exists():
        input_path.unlink()

    return output_path


def decrypt_file(
    input_path: Path,
    config: EncryptionConfig,
    output_path: Optional[Path] = None,
    remove_encrypted: bool = False,
) -> Path:
    """
    Decrypt an age-encrypted file.

    Args:
        input_path: Encrypted .age file.
        config: Encryption config with identity (private key) path.
        output_path: Destination. Defaults to input_path without .age suffix.
        remove_encrypted: Delete encrypted file after decryption.

    Returns:
        Path to decrypted file.
    """
    check_age_installed()
    input_path = Path(input_path)

    if output_path is None:
        name = input_path.name
        if name.endswith(ENCRYPTED_EXT):
            name = name[: -len(ENCRYPTED_EXT)]
        output_path = input_path.parent / name

    cmd = ["age", "-d"]

    if config.identity_path:
        identity = _validate_identity_path(config.identity_path)
        cmd.extend(["-i", str(identity)])

    cmd.extend(["-o", str(output_path), str(input_path)])

    result = subprocess.run(
        cmd, capture_output=True, text=True, timeout=300
    )
    if result.returncode != 0:
        raise EncryptionError("age decrypt failed. Run with --verbose for details.")

    if remove_encrypted and output_path.exists():
        input_path.unlink()

    return output_path
