---
title: "My AI Had 4,538 Files of Amnesia. I Built the Fix."
date: 2026-03-18
author: "Kevin Qi"
tags: [engram, ai-memory, compression, post-quantum, security, parquet, context-window, deepseek]
source: "original"
fact_checked: true
ai_assisted: true
---

# My AI Had 4,538 Files of Amnesia. I Built the Fix.

Your AI assistant forgets everything.

Every session starts from zero. The context window fills up. That architecture decision from January? Gone. The bug you spent an hour explaining last month? Never happened.

The workaround is memory files. Claude writes to `~/.claude/`. ChatGPT keeps desktop caches. Cursor logs conversations. Six months of daily use and here's what was sitting on my machine:

- **4,538 files** consuming **2.6 GB**
- **Zero** encrypted
- **Zero** compressed
- **Zero** searchable

Every code review, security discussion, and personal decision — stored in plaintext, retrievable by anyone with access to the disk.

One command changed that:

```
$ engram run

4,537 artifacts tracked and indexed
609 compressed across warm and cold tiers
11.58x compression ratio (real data, not benchmarks)
131,271 keywords searchable across all tiers
1.6 GB reclaimed
Encryption: per-artifact keys, private keys in Keychain (Touch ID)
```

The disk savings aren't the point. The point is that my AI assistant now references decisions I made three months ago without me re-explaining them. The context window didn't get bigger. The memory got smarter.

## The brain already solved this

Your brain doesn't hold everything in working memory. It tiers. Recent experiences stay vivid and fast. Older memories compress into patterns. Deep memories take the right cue to surface.

I applied the same architecture:

| Brain System | Engram Tier | Compression | Retrieval |
|---|---|---|---|
| Working memory | **Hot** | 1x (raw) | Instant |
| Recent memory | **Warm** | **4-5x** | ~10ms |
| Long-term memory | **Cold** | **8-12x** | ~500ms |
| Deep memory | **Frozen** | **20-50x** | ~5 seconds |

Those ratios aren't from cranking zstd levels (that gets 3.2x to 3.8x). Each tier applies different data transformations before the compressor runs. Warm minifies JSON. Cold strips boilerplate and trains a dictionary on your sessions. Frozen converts JSONL to columnar Parquet — the same format that lets ClickHouse achieve [170x compression](https://clickhouse.com/blog/log-compression-170x) on logs.

**Only the actual content carries real entropy. Everything else compresses away.**

## The principle, not the technique

DeepSeek-V2 ([arXiv:2405.04434](https://arxiv.org/abs/2405.04434)) showed that compressing what goes into the model's attention — caching small latent vectors instead of recalculating full key/value tensors — cut their KV cache by 93.3% and improved throughput 5.76x. They solved it inside the model.

Engram solves it outside the model. Different layer of the stack, same principle: don't keep recalculating or reloading what you can store compressed and recall on demand. DeepSeek compressed the attention cache. Engram compresses the context memory. Both save hardware resources at scale by being smarter about what stays in memory and what gets stored efficiently.

## The search problem

Compression without search is a write-only archive. If you can't find a memory, it doesn't matter how efficiently it's stored.

Every artifact gets indexed before compression. Keywords extracted. Summary generated. The index is under 1 MB for thousands of artifacts. Always loaded. Never compressed. It's the hippocampus of the system.

When your AI starts a session, Engram feeds it a budget-optimized block of relevant summaries — not full files. If a summary isn't enough, the assistant explicitly recalls the full artifact. The AI never decompresses everything hoping to find something.

## Your sessions are a target

NIST proposed deprecating RSA-2048 by 2030 ([IR 8547](https://nvlpubs.nist.gov/nistpubs/ir/2024/NIST.IR.8547.ipd.pdf)). An adversary who captures your plaintext session files today can wait for quantum computers. That's harvest-now-decrypt-later.

Engram's encryption uses ML-KEM-768 (NIST [FIPS 203](https://csrc.nist.gov/pubs/fips/203/final)), the same post-quantum algorithm [OpenSSH 10.0 made the default](https://www.openssh.org/pq.html) in April 2025. Private keys are handled by a compiled Rust sidecar with `mlock` and `zeroize` — they never enter Python's memory, never touch disk, never appear in process args.

Your keys live in Keychain or Vault. Never as files. If you lose the key, data is gone. That's the point.

## What makes this different

- **Real compression, not flat zstd.** Multi-stage pipeline: 4-5x / 8-12x / 20-50x per tier. Real result: 2.6 GB → 1.6 GB saved on actual session data.
- **Search without decompression.** Semantic index searches all tiers instantly. 132K keywords indexed.
- **Post-quantum encryption.** Per-artifact keys. Per-tier keypairs. Rust crypto sidecar. Keys never in Python.
- **AI-agnostic.** Claude, Codex, ChatGPT, Cursor, Copilot, OpenClaw. Add any directory.
- **Everything local.** Zero telemetry. No cloud. Your data stays yours.

## See what you're wasting in 30 seconds

```bash
pip install engram
engram init           # auto-detects your AI assistant locations
engram run --dry-run  # shows what would be compressed — changes nothing
```

The dry run is free. It scans your disk, shows you the file count, total size, and what would move to each tier. No files are modified until you run `engram run`.

Then when you're ready:

```bash
engram run                               # compress + encrypt
engram search "that auth refactor"       # search all tiers
engram context --query "auth patterns"   # context block for your AI
```

72 tests. 7 rounds of security review. [Full threat model, security recommendations, and architecture docs on GitHub.](https://github.com/qinnovates/engram)

Open source. MIT license.

**[github.com/qinnovates/engram](https://github.com/qinnovates/engram)**

---

*Written with AI assistance (Claude). All claims verified against primary sources. The author takes full responsibility for all content.*
