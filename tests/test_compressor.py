"""Tests for the compression engine."""

import os
import tempfile
from pathlib import Path

import pytest

from src.compressor import (
    compress_file,
    decompress_file,
    recompress_file,
    CompressionResult,
)
from src.config import WARM_COMPRESSION_LEVEL, COLD_COMPRESSION_LEVEL


@pytest.fixture
def sample_file(tmp_path: Path) -> Path:
    """Create a sample file with repetitive content (compressible)."""
    f = tmp_path / "sample.jsonl"
    # Repetitive JSON lines — similar to real subagent logs
    lines = []
    for i in range(500):
        lines.append(
            f'{{"id": {i}, "role": "assistant", "content": "This is a test message '
            f'with repetitive content for compression testing purposes number {i}"}}\n'
        )
    f.write_text("".join(lines))
    return f


@pytest.fixture
def small_file(tmp_path: Path) -> Path:
    """Create a small file (below min threshold)."""
    f = tmp_path / "tiny.txt"
    f.write_text("hello")
    return f


class TestCompressFile:
    def test_basic_compression(self, sample_file: Path):
        result = compress_file(sample_file, level=WARM_COMPRESSION_LEVEL)
        assert result.output_path.exists()
        assert result.compressed_size < result.original_size
        assert result.ratio > 1.0
        assert result.level == WARM_COMPRESSION_LEVEL

    def test_output_path_default(self, sample_file: Path):
        result = compress_file(sample_file)
        assert result.output_path.name == "sample.jsonl.zst"

    def test_custom_output_path(self, sample_file: Path, tmp_path: Path):
        out = tmp_path / "custom.zst"
        result = compress_file(sample_file, output_path=out)
        assert result.output_path == out
        assert out.exists()

    def test_remove_original(self, sample_file: Path):
        compress_file(sample_file, remove_original=True)
        assert not sample_file.exists()

    def test_keep_original(self, sample_file: Path):
        compress_file(sample_file, remove_original=False)
        assert sample_file.exists()

    def test_warm_vs_cold_ratio(self, sample_file: Path, tmp_path: Path):
        warm_result = compress_file(
            sample_file,
            output_path=tmp_path / "warm.zst",
            level=WARM_COMPRESSION_LEVEL,
        )
        cold_result = compress_file(
            sample_file,
            output_path=tmp_path / "cold.zst",
            level=COLD_COMPRESSION_LEVEL,
        )
        # Cold should be smaller or equal (higher compression)
        assert cold_result.compressed_size <= warm_result.compressed_size

    def test_missing_file_raises(self, tmp_path: Path):
        with pytest.raises(FileNotFoundError):
            compress_file(tmp_path / "nonexistent.txt")


class TestDecompressFile:
    def test_roundtrip(self, sample_file: Path, tmp_path: Path):
        original_content = sample_file.read_bytes()
        compressed = compress_file(sample_file, output_path=tmp_path / "test.zst")
        decompressed = decompress_file(
            compressed.output_path,
            output_path=tmp_path / "restored.jsonl",
        )
        assert decompressed.read_bytes() == original_content

    def test_remove_compressed(self, sample_file: Path, tmp_path: Path):
        result = compress_file(sample_file, output_path=tmp_path / "test.zst")
        decompress_file(result.output_path, remove_compressed=True)
        assert not result.output_path.exists()

    def test_missing_file_raises(self, tmp_path: Path):
        with pytest.raises(FileNotFoundError):
            decompress_file(tmp_path / "missing.zst")


class TestRecompressFile:
    def test_warm_to_cold_recompress(self, sample_file: Path, tmp_path: Path):
        # First compress at warm level
        warm = compress_file(
            sample_file,
            output_path=tmp_path / "test.zst",
            level=WARM_COMPRESSION_LEVEL,
        )
        warm_size = warm.compressed_size

        # Recompress at cold level
        cold = recompress_file(warm.output_path, new_level=COLD_COMPRESSION_LEVEL)
        assert cold.compressed_size <= warm_size
        assert cold.level == COLD_COMPRESSION_LEVEL

    def test_recompressed_content_intact(self, sample_file: Path, tmp_path: Path):
        original = sample_file.read_bytes()
        warm = compress_file(
            sample_file,
            output_path=tmp_path / "test.zst",
            level=WARM_COMPRESSION_LEVEL,
        )
        recompress_file(warm.output_path, new_level=COLD_COMPRESSION_LEVEL)
        restored = decompress_file(
            warm.output_path,
            output_path=tmp_path / "restored.jsonl",
        )
        assert restored.read_bytes() == original
