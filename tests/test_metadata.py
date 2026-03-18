"""Tests for the metadata store."""

import time
from pathlib import Path

import pytest

from src.metadata import MetadataStore, Tier, ArtifactMeta, compute_sha256


@pytest.fixture
def meta_store(tmp_path: Path) -> MetadataStore:
    return MetadataStore(tmp_path / "meta")


@pytest.fixture
def sample_file(tmp_path: Path) -> Path:
    f = tmp_path / "test.jsonl"
    f.write_text('{"test": true}\n' * 100)
    return f


class TestMetadataStore:
    def test_register_new_file(self, meta_store: MetadataStore, sample_file: Path):
        meta = meta_store.register(sample_file)
        assert meta.tier == Tier.HOT.value
        assert meta.original_size > 0
        assert meta.last_accessed > 0

    def test_register_idempotent(self, meta_store: MetadataStore, sample_file: Path):
        m1 = meta_store.register(sample_file)
        m2 = meta_store.register(sample_file)
        assert m1.path == m2.path
        # Second register updates access time
        assert m2.last_accessed >= m1.last_accessed

    def test_save_and_reload(self, tmp_path: Path, sample_file: Path):
        store1 = MetadataStore(tmp_path / "meta")
        store1.register(sample_file)
        store1.save()

        store2 = MetadataStore(tmp_path / "meta")
        meta = store2.get(sample_file)
        assert meta is not None
        assert meta.path == str(sample_file.resolve())

    def test_touch_updates_access(self, meta_store: MetadataStore, sample_file: Path):
        meta_store.register(sample_file)
        old_access = meta_store.get(sample_file).last_accessed
        time.sleep(0.01)
        meta_store.touch(sample_file)
        new_access = meta_store.get(sample_file).last_accessed
        assert new_access > old_access

    def test_update_tier(self, meta_store: MetadataStore, sample_file: Path):
        meta_store.register(sample_file)
        meta_store.update_tier(sample_file, Tier.WARM, compressed_size=100, ratio=5.0)
        meta = meta_store.get(sample_file)
        assert meta.tier == Tier.WARM.value
        assert meta.compressed_size == 100
        assert meta.compression_ratio == 5.0

    def test_by_tier(self, meta_store: MetadataStore, tmp_path: Path):
        f1 = tmp_path / "hot.txt"
        f1.write_text("hot")
        f2 = tmp_path / "warm.txt"
        f2.write_text("warm")

        meta_store.register(f1)
        meta_store.register(f2)
        meta_store.update_tier(f2, Tier.WARM)

        assert len(meta_store.by_tier(Tier.HOT)) == 1
        assert len(meta_store.by_tier(Tier.WARM)) == 1

    def test_candidates_for_warm(self, meta_store: MetadataStore, sample_file: Path):
        meta = meta_store.register(sample_file)
        # Artificially age the artifact
        meta.created_at = time.time() - (72 * 3600)  # 72 hours ago
        meta.last_accessed = time.time() - (48 * 3600)  # 48 hours ago

        candidates = meta_store.candidates_for_warm(
            age_hours=48, idle_hours=24, min_size=10
        )
        assert len(candidates) == 1

    def test_stats(self, meta_store: MetadataStore, sample_file: Path):
        meta_store.register(sample_file)
        stats = meta_store.stats()
        assert stats["total_artifacts"] == 1
        assert stats["hot_count"] == 1
        assert stats["warm_count"] == 0

    def test_remove(self, meta_store: MetadataStore, sample_file: Path):
        meta_store.register(sample_file)
        meta_store.remove(sample_file)
        assert meta_store.get(sample_file) is None


class TestComputeSha256:
    def test_hash_deterministic(self, sample_file: Path):
        h1 = compute_sha256(sample_file)
        h2 = compute_sha256(sample_file)
        assert h1 == h2
        assert len(h1) == 64  # SHA-256 hex digest

    def test_hash_changes_on_content(self, tmp_path: Path):
        f = tmp_path / "test.txt"
        f.write_text("version 1")
        h1 = compute_sha256(f)
        f.write_text("version 2")
        h2 = compute_sha256(f)
        assert h1 != h2
