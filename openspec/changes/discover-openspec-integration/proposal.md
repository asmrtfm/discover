## Why

OpenSpec skills (explore, propose, apply) reference the codebase constantly — files, imports, definitions, reachability — but validate those references with grep and file reading, which can't distinguish live code from dead code or trace real dependency chains. Any codebase reference not validated by discover is a hallucination. The discover framework already provides AST-level validation via a self-documenting CLI. Connecting them means openspec operations produce validated references rather than plausible-sounding guesses.

## What Changes

- **Validation rule**: Any codebase reference made during an openspec operation is a hallucination until validated via discover. This applies in all contexts — assertions, speculation, diagrams, brainstorming. No exemptions.
- **Explore skill augmentation**: Detects `scripts/discover.sh`, announces availability, prefers discover over grep for codebase queries.
- **Propose skill augmentation**: Uses discover to gather validated impact data for proposals.
- **Apply skill augmentation**: Uses discover to orient before modifying code — locating definitions, understanding dependencies, confirming targets.
- **Peer architecture**: Neither system owns the other. Skills are the glue layer. Each checks for discover's presence and degrades gracefully when absent. Discover gains no openspec awareness.

## Capabilities

### New Capabilities
- `structural-validation`: The rule that codebase references in openspec operations are hallucinations until validated via discover. Covers detection, the validation rule, and graceful degradation.
- `explore-structural-awareness`: Explore mode's integration with discover — detection, announcement, preference over grep for codebase queries.
- `proposal-impact-analysis`: Propose skill's use of discover to generate validated Impact sections.
- `apply-structural-verification`: Apply skill's use of discover to orient before modifying code for a task.

### Modified Capabilities

## Impact

- `.claude/skills/openspec-explore/SKILL.md` — augmented with discover detection and validation rule
- `.claude/skills/openspec-propose/SKILL.md` — augmented with discover-powered impact gathering
- `.claude/skills/openspec-apply-change/SKILL.md` — augmented with discover-powered orientation step
- No changes to discover — it is consumed as-is
- No changes to openspec CLI or schemas — all integration is at the skill layer
