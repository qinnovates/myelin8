"""
Compression engine using zstandard (zstd).

Tier compression strategy (modeled after Splunk/ELK production patterns):
  WARM: zstd level 3  — balanced ratio/speed (~3.2x, ~234 MB/s)
  COLD: zstd level 9  — maximum practical ratio (~3.5x, ~40 MB/s)

Why zstd:
  - Default in Splunk 7.2+, Elasticsearch 8.17+, Apache Iceberg 1.4+
  - Beats gzip on both ratio AND speed at equivalent levels
  - Dictionary training possible for repetitive data (future enhancement)
  - Decompression speed ~1,550 MB/s regardless of compression level
"""

from __future__ import annotations

import os
import tempfile
from pathlib import Path
from typing import Optional

import zstandard as zstd

from .config import WARM_COMPRESSION_LEVEL, COLD_COMPRESSION_LEVEL, COMPRESSED_EXT


class CompressionResult:
    """Result of a compression operation."""
    def __init__(self, original_size: int, compressed_size: int,
                 output_path: Path, level: int):
        self.original_size = original_size
        self.compressed_size = compressed_size
        self.output_path = output_path
        self.level = level

    @property
    def ratio(self) -> float:
        if self.compressed_size == 0:
            return 0.0
        return round(self.original_size / self.compressed_size, 2)

    @property
    def savings_pct(self) -> float:
        if self.original_size == 0:
            return 0.0
        return round((1 - self.compressed_size / self.original_size) * 100, 1)

    def __repr__(self) -> str:
        return (
            f"CompressionResult(ratio={self.ratio}x, "
            f"saved={self.savings_pct}%, "
            f"level={self.level}, "
            f"{self.original_size} -> {self.compressed_size} bytes)"
        )


def _open_temp_restricted(directory: Path, suffix: str = ".tmp") -> tuple[int, Path]:
    """Create a temp file with 0600 permissions. Returns (fd, path)."""
    fd, path_str = tempfile.mkstemp(suffix=suffix, dir=str(directory))
    return fd, Path(path_str)


def compress_file(
    input_path: Path,
    output_path: Optional[Path] = None,
    level: int = WARM_COMPRESSION_LEVEL,
    remove_original: bool = False,
) -> CompressionResult:
    """
    Compress a file using zstd at the specified level.

    Args:
        input_path: File to compress.
        output_path: Destination. Defaults to input_path + .zst
        level: zstd compression level (1-22). Use WARM_COMPRESSION_LEVEL (3)
               or COLD_COMPRESSION_LEVEL (9).
        remove_original: Delete source file after successful compression.

    Returns:
        CompressionResult with sizes and ratio.
    """
    input_path = Path(input_path)
    if not input_path.exists():
        raise FileNotFoundError(f"Source file not found: {input_path}")

    if output_path is None:
        output_path = input_path.with_suffix(input_path.suffix + COMPRESSED_EXT)
    output_path = Path(output_path)

    original_size = input_path.stat().st_size
    compressor = zstd.ZstdCompressor(level=level)

    # Use unpredictable temp file with restricted permissions
    fd, tmp_path = _open_temp_restricted(output_path.parent, suffix=".zst.tmp")
    try:
        with open(input_path, "rb") as fin, os.fdopen(fd, "wb") as fout:
            compressor.copy_stream(fin, fout)
        # Atomic rename
        tmp_path.rename(output_path)
    except Exception:
        # Clean up temp on failure
        if tmp_path.exists():
            tmp_path.unlink()
        raise

    compressed_size = output_path.stat().st_size

    if remove_original and output_path.exists():
        input_path.unlink()

    return CompressionResult(
        original_size=original_size,
        compressed_size=compressed_size,
        output_path=output_path,
        level=level,
    )


def decompress_file(
    input_path: Path,
    output_path: Optional[Path] = None,
    remove_compressed: bool = False,
) -> Path:
    """
    Decompress a .zst file back to its original form.

    Args:
        input_path: Compressed .zst file.
        output_path: Destination. Defaults to input_path without .zst suffix.
        remove_compressed: Delete compressed file after decompression.

    Returns:
        Path to decompressed file.
    """
    input_path = Path(input_path)
    if not input_path.exists():
        raise FileNotFoundError(f"Compressed file not found: {input_path}")

    if output_path is None:
        # Strip the .zst extension
        name = input_path.name
        if name.endswith(COMPRESSED_EXT):
            name = name[: -len(COMPRESSED_EXT)]
        output_path = input_path.parent / name
    output_path = Path(output_path)

    decompressor = zstd.ZstdDecompressor()

    fd, tmp_path = _open_temp_restricted(output_path.parent, suffix=".dec.tmp")
    try:
        with open(input_path, "rb") as fin, os.fdopen(fd, "wb") as fout:
            decompressor.copy_stream(fin, fout)
        tmp_path.rename(output_path)
    except Exception:
        if tmp_path.exists():
            tmp_path.unlink()
        raise

    if remove_compressed:
        input_path.unlink()

    return output_path


def recompress_file(
    input_path: Path,
    new_level: int = COLD_COMPRESSION_LEVEL,
) -> CompressionResult:
    """
    Recompress a warm-tier file at a higher compression level for cold storage.

    This is the warm -> cold transition: decompress from level 3, recompress at level 9.
    Matches the ELK pattern of re-indexing with best_compression codec during ILM phase transition.

    Uses streaming to avoid loading entire file into memory (decompression bomb safe).
    All temp files use unpredictable names and restricted permissions.

    Args:
        input_path: Currently compressed .zst file.
        new_level: Target compression level (default: COLD_COMPRESSION_LEVEL = 9).

    Returns:
        CompressionResult with new sizes.
    """
    input_path = Path(input_path)

    decompressor = zstd.ZstdDecompressor()
    compressor = zstd.ZstdCompressor(level=new_level)

    raw_fd, raw_tmp = _open_temp_restricted(input_path.parent, suffix=".raw.tmp")
    recomp_fd = None
    recomp_tmp = None

    try:
        # Step 1: Decompress to temp (streaming)
        with open(input_path, "rb") as fin, os.fdopen(raw_fd, "wb") as fout:
            decompressor.copy_stream(fin, fout)

        original_size = raw_tmp.stat().st_size

        # Step 2: Recompress at higher level
        recomp_fd, recomp_tmp = _open_temp_restricted(input_path.parent, suffix=".recomp.tmp")
        with open(raw_tmp, "rb") as fin, os.fdopen(recomp_fd, "wb") as fout:
            compressor.copy_stream(fin, fout)

        compressed_size = recomp_tmp.stat().st_size

        # Clean up raw temp, atomic rename recompressed
        raw_tmp.unlink()
        recomp_tmp.rename(input_path)

    except Exception:
        # Clean up both temps on failure
        if raw_tmp and raw_tmp.exists():
            raw_tmp.unlink()
        if recomp_tmp and recomp_tmp.exists():
            recomp_tmp.unlink()
        raise

    return CompressionResult(
        original_size=original_size,
        compressed_size=compressed_size,
        output_path=input_path,
        level=new_level,
    )
