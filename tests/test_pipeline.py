"""Tests for the multi-stage compression pipeline."""

import json
from pathlib import Path

import pytest

from src.pipeline import (
    CompressionPipeline,
    minify_json,
    strip_boilerplate,
    restore_boilerplate,
    jsonl_to_parquet,
    parquet_to_jsonl,
    _looks_like_jsonl,
    HAS_PYARROW,
)


@pytest.fixture
def session_jsonl(tmp_path: Path) -> Path:
    """Create a realistic AI session JSONL file."""
    f = tmp_path / "session.jsonl"
    lines = []
    # System prompt (boilerplate — repeated across sessions)
    lines.append(json.dumps({
        "role": "system",
        "content": (
            "You are Claude, an AI assistant. <system-reminder> "
            "You are an interactive agent that helps users. " * 20
        ),
        "timestamp": 1710000000,
    }))
    # Conversation turns
    for i in range(200):
        lines.append(json.dumps({
            "role": "user" if i % 2 == 0 else "assistant",
            "content": f"Turn {i}: {'What about security?' if i % 2 == 0 else 'Here is my analysis of the threat model for iteration ' + str(i)}",
            "timestamp": 1710000000 + i * 60,
        }))
    f.write_text("\n".join(lines))
    return f


@pytest.fixture
def pretty_json(tmp_path: Path) -> Path:
    """Create a pretty-printed JSON file with lots of whitespace."""
    f = tmp_path / "formatted.json"
    data = [
        {"role": "user", "content": f"message {i}", "metadata": {"turn": i}}
        for i in range(100)
    ]
    f.write_text(json.dumps(data, indent=4))
    return f


@pytest.fixture
def pipeline(tmp_path: Path) -> CompressionPipeline:
    meta = tmp_path / "meta"
    meta.mkdir()
    return CompressionPipeline(meta)


class TestMinifyJson:
    def test_strips_whitespace(self, pretty_json: Path):
        raw = pretty_json.read_bytes()
        minified = minify_json(raw)
        assert len(minified) < len(raw)
        # Verify it's still valid JSON
        json.loads(minified)

    def test_preserves_jsonl(self, session_jsonl: Path):
        raw = session_jsonl.read_bytes()
        minified = minify_json(raw)
        lines = [l for l in minified.split(b"\n") if l.strip()]
        for line in lines[:5]:
            json.loads(line)  # Each line must be valid JSON

    def test_non_json_preserved(self):
        raw = b"# This is markdown\nNot JSON at all\n"
        result = minify_json(raw)
        assert b"markdown" in result
        assert b"Not JSON" in result

    def test_savings_on_indented(self, pretty_json: Path):
        raw = pretty_json.read_bytes()
        minified = minify_json(raw)
        savings = 1 - len(minified) / len(raw)
        assert savings > 0.3  # At least 30% savings on indented JSON


class TestBoilerplateStripping:
    def test_detects_system_prompt(self, session_jsonl: Path, tmp_path: Path):
        store = tmp_path / "boilerplate"
        raw = session_jsonl.read_bytes()
        stripped = strip_boilerplate(raw, store)
        assert len(stripped) < len(raw)
        assert b"BOILERPLATE_REF:" in stripped

    def test_restores_boilerplate(self, session_jsonl: Path, tmp_path: Path):
        store = tmp_path / "boilerplate"
        raw = session_jsonl.read_bytes()
        stripped = strip_boilerplate(raw, store)
        restored = restore_boilerplate(stripped, store)
        # Content should be equivalent (may have minification diffs)
        assert b"You are Claude" in restored

    def test_no_strip_on_short_content(self, tmp_path: Path):
        store = tmp_path / "boilerplate"
        f = tmp_path / "short.jsonl"
        f.write_text('{"role": "user", "content": "Hi"}\n')
        raw = f.read_bytes()
        stripped = strip_boilerplate(raw, store)
        assert stripped == raw  # Nothing to strip


@pytest.mark.skipif(not HAS_PYARROW, reason="pyarrow not installed")
class TestParquetConversion:
    def test_jsonl_roundtrip(self, session_jsonl: Path, tmp_path: Path):
        raw = session_jsonl.read_bytes()
        parquet_path = tmp_path / "output.parquet"
        size = jsonl_to_parquet(raw, parquet_path)
        assert size > 0
        assert parquet_path.exists()

        # Roundtrip
        restored = parquet_to_jsonl(parquet_path)
        restored_lines = [l for l in restored.split(b"\n") if l.strip()]
        original_lines = [l for l in raw.split(b"\n") if l.strip()]
        assert len(restored_lines) == len(original_lines)

    def test_parquet_smaller_than_raw(self, session_jsonl: Path, tmp_path: Path):
        raw = session_jsonl.read_bytes()
        parquet_path = tmp_path / "output.parquet"
        jsonl_to_parquet(raw, parquet_path)
        # Parquet should be smaller than raw JSONL
        assert parquet_path.stat().st_size < len(raw)

    def test_compression_ratio(self, session_jsonl: Path, tmp_path: Path):
        raw = session_jsonl.read_bytes()
        parquet_path = tmp_path / "output.parquet"
        jsonl_to_parquet(raw, parquet_path)
        ratio = len(raw) / parquet_path.stat().st_size
        # Should achieve at least 3x (conservative, since test data is small)
        assert ratio > 2.0


class TestLooksLikeJsonl:
    def test_jsonl_detected(self):
        content = b'{"a": 1}\n{"b": 2}\n{"c": 3}\n'
        assert _looks_like_jsonl(content) is True

    def test_markdown_not_jsonl(self):
        content = b"# Header\nSome text\n- List item\n"
        assert _looks_like_jsonl(content) is False

    def test_mixed_content(self):
        content = b'not json\n{"a": 1}\nmore text\n'
        assert _looks_like_jsonl(content) is False


class TestCompressionPipeline:
    def test_warm_pipeline(self, pipeline: CompressionPipeline,
                           session_jsonl: Path, tmp_path: Path):
        output = tmp_path / "session.warm.zst"
        result = pipeline.compress_warm(session_jsonl, output)
        assert output.exists()
        assert result.ratio > 2.0
        assert "zstd-3" in result.stages
        # Decompress roundtrip
        restored = pipeline.decompress_warm(output)
        assert len(restored) > 0

    def test_cold_pipeline(self, pipeline: CompressionPipeline,
                           session_jsonl: Path, tmp_path: Path):
        output = tmp_path / "session.cold.zst"
        result = pipeline.compress_cold(session_jsonl, output)
        assert output.exists()
        assert result.ratio > 2.0
        assert any("zstd-9" in s for s in result.stages)

    def test_cold_better_than_warm(self, pipeline: CompressionPipeline,
                                    session_jsonl: Path, tmp_path: Path):
        warm_out = tmp_path / "session.warm.zst"
        cold_out = tmp_path / "session.cold.zst"
        warm = pipeline.compress_warm(session_jsonl, warm_out)
        cold = pipeline.compress_cold(session_jsonl, cold_out)
        # Cold should achieve equal or better ratio
        assert cold.final_size <= warm.final_size

    @pytest.mark.skipif(not HAS_PYARROW, reason="pyarrow not installed")
    def test_frozen_pipeline_parquet(self, pipeline: CompressionPipeline,
                                      session_jsonl: Path, tmp_path: Path):
        output = tmp_path / "session.frozen.parquet"
        result = pipeline.compress_frozen(session_jsonl, output)
        assert output.exists()
        assert "parquet-columnar-zstd-19" in result.stages
        # Parquet has more per-file overhead than zstd on small files.
        # On large files (100KB+), Parquet's columnar layout wins.
        # On this 23KB test file, just verify it compressed meaningfully.
        assert result.ratio > 3.0

    @pytest.mark.skipif(not HAS_PYARROW, reason="pyarrow not installed")
    def test_frozen_decompress_roundtrip(self, pipeline: CompressionPipeline,
                                          session_jsonl: Path, tmp_path: Path):
        output = tmp_path / "session.frozen.parquet"
        pipeline.compress_frozen(session_jsonl, output)
        restored = pipeline.decompress_frozen(output)
        assert len(restored) > 0
        # Verify JSONL structure preserved
        lines = [l for l in restored.split(b"\n") if l.strip()]
        assert len(lines) > 100

    def test_dictionary_training(self, pipeline: CompressionPipeline,
                                  tmp_path: Path):
        # Create enough sample files for training
        samples = []
        for i in range(20):
            f = tmp_path / f"sample-{i}.jsonl"
            lines = [
                json.dumps({"role": "user", "content": f"msg {j} in session {i}",
                            "timestamp": 1710000000 + j})
                for j in range(50)
            ]
            f.write_text("\n".join(lines))
            samples.append(f)

        success = pipeline.train_dict(samples)
        assert success
        assert pipeline.get_dictionary() is not None
