"""Tests for Merkle tree via Rust sidecar (myelin8-vault)."""

import pytest

from src.vault import VaultClient
from src.encryption import EncryptionError


@pytest.fixture
def vault():
    """Create a VaultClient connected to the sidecar."""
    try:
        client = VaultClient()
        client.merkle_reset()  # Start fresh
        yield client
        client.close()
    except EncryptionError:
        pytest.skip("myelin8-vault sidecar not available")


class TestMerkleAdd:

    def test_add_returns_index(self, vault):
        idx = vault.merkle_add("a" * 64)
        assert idx == 0

    def test_sequential_indices(self, vault):
        idx0 = vault.merkle_add("a" * 64)
        idx1 = vault.merkle_add("b" * 64)
        idx2 = vault.merkle_add("c" * 64)
        assert idx0 == 0
        assert idx1 == 1
        assert idx2 == 2

    def test_count_tracks_adds(self, vault):
        assert vault.merkle_count() == 0
        vault.merkle_add("a" * 64)
        assert vault.merkle_count() == 1
        vault.merkle_add("b" * 64)
        assert vault.merkle_count() == 2

    def test_invalid_hex_rejected(self, vault):
        with pytest.raises(EncryptionError):
            vault.merkle_add("not_hex")

    def test_wrong_length_rejected(self, vault):
        with pytest.raises(EncryptionError):
            vault.merkle_add("abcd")  # Too short


class TestMerkleRoot:

    def test_empty_tree_returns_none(self, vault):
        assert vault.merkle_root() is None

    def test_root_after_add(self, vault):
        vault.merkle_add("a" * 64)
        root = vault.merkle_root()
        assert root is not None
        assert len(root) == 64  # SHA-256 hex

    def test_root_changes_with_new_leaf(self, vault):
        vault.merkle_add("a" * 64)
        root1 = vault.merkle_root()
        vault.merkle_add("b" * 64)
        root2 = vault.merkle_root()
        assert root1 != root2

    def test_deterministic_root(self, vault):
        vault.merkle_add("a" * 64)
        vault.merkle_add("b" * 64)
        root1 = vault.merkle_root()

        vault.merkle_reset()
        vault.merkle_add("a" * 64)
        vault.merkle_add("b" * 64)
        root2 = vault.merkle_root()

        assert root1 == root2

    def test_different_order_different_root(self, vault):
        vault.merkle_add("a" * 64)
        vault.merkle_add("b" * 64)
        root1 = vault.merkle_root()

        vault.merkle_reset()
        vault.merkle_add("b" * 64)
        vault.merkle_add("a" * 64)
        root2 = vault.merkle_root()

        assert root1 != root2


class TestMerkleProof:

    def test_proof_for_first_leaf(self, vault):
        vault.merkle_add("a" * 64)
        vault.merkle_add("b" * 64)
        vault.merkle_add("c" * 64)
        vault.merkle_add("d" * 64)

        proof = vault.merkle_proof(0)
        assert proof["leaf_index"] == 0
        assert vault.merkle_verify(proof)

    def test_proof_for_last_leaf(self, vault):
        for i in range(8):
            vault.merkle_add(f"{i:064x}")

        proof = vault.merkle_proof(7)
        assert vault.merkle_verify(proof)

    def test_proof_fails_with_wrong_root(self, vault):
        vault.merkle_add("a" * 64)
        vault.merkle_add("b" * 64)

        proof = vault.merkle_proof(0)
        proof["root"] = "0" * 64  # Fake root
        assert not vault.merkle_verify(proof)

    def test_proof_fails_with_tampered_leaf(self, vault):
        vault.merkle_add("a" * 64)
        vault.merkle_add("b" * 64)

        proof = vault.merkle_proof(0)
        proof["leaf_hash"] = "0" * 64  # Tampered
        assert not vault.merkle_verify(proof)

    def test_odd_number_of_leaves(self, vault):
        vault.merkle_add("a" * 64)
        vault.merkle_add("b" * 64)
        vault.merkle_add("c" * 64)  # 3 = not power of 2

        for i in range(3):
            proof = vault.merkle_proof(i)
            assert vault.merkle_verify(proof)

    def test_single_leaf_proof(self, vault):
        vault.merkle_add("a" * 64)
        proof = vault.merkle_proof(0)
        assert vault.merkle_verify(proof)


class TestMerkleReset:

    def test_reset_clears_tree(self, vault):
        vault.merkle_add("a" * 64)
        vault.merkle_add("b" * 64)
        assert vault.merkle_count() == 2

        vault.merkle_reset()
        assert vault.merkle_count() == 0
        assert vault.merkle_root() is None
