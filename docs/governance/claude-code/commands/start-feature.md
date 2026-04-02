# /start-feature

Use this command when beginning any new feature in this repo.

## Steps

1. **Intake** — state in one sentence what the feature does and what's out of scope.

2. **Identify which layer(s) are affected:**
   - New config field → which layer does it belong in?
   - New schema → does it require changes to the memory write gate?
   - New template → does it introduce a new precedence interaction?
   - Documentation → which docs section does it update?

3. **Check for existing coverage:**
   ```
   grep -rn "<feature_keyword>" schemas/ templates/ docs/
   ```
   If it exists in some form — extend it, don't duplicate it.

4. **Precedence impact assessment:**
   - Does this field need `overridable:` annotation?
   - Can a downstream layer break safety if this field is overridden?
   - Does this change the context assembly algorithm?

5. **Build order:**
   1. Schema change (if any) — define the shape first
   2. Template update — implement the new field in the appropriate layer
   3. Documentation update — explain the what and why
   4. Hook update (if security-relevant) — add semgrep rule if needed
   5. Test — add test cases for the new behavior

6. **Acceptance criteria** — state 3 verifiable conditions for "done."
