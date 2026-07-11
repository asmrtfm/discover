## Context

Three openspec skills (explore, propose, apply) operate on codebases that may have discover installed. Discover is a self-documenting bash CLI at `scripts/discover.sh` with `--help` for subcommand discovery. The skills are Claude Code SKILL.md files — plain markdown that shapes agent behavior. The integration is entirely at the skill instruction layer.

Current skill files:
- `.claude/skills/openspec-explore/SKILL.md`
- `.claude/skills/openspec-propose/SKILL.md`
- `.claude/skills/openspec-apply-change/SKILL.md`

Discover is installed per-project via `discover init <lang>`, producing `scripts/discover.sh`.

## Goals / Non-Goals

**Goals:**
- Every codebase reference in an openspec operation is validated via discover when available
- Skills detect discover once per invocation and degrade gracefully when absent
- Discover remains a black box to the skills — they use `--help`, not hardcoded subcommand knowledge

**Non-Goals:**
- Modifying discover to be openspec-aware
- Adding openspec CLI commands or schema changes
- Hardcoding discover's subcommand interface into skill instructions

## Decisions

### Skill augmentation via instruction blocks, not new skills
Each existing skill gets a discover-awareness section added to its SKILL.md. No new skills are created.

**Why over separate skills:** A separate "discover-validate" skill would require the agent to context-switch between skills. Embedding the rule in each skill makes it inescapable — the agent reads it as part of the skill it's already executing.

**Why over a shared include:** SKILL.md files don't support includes. Duplication across three files is acceptable given the rule is short.

### Detection via file existence check
Skills detect discover by checking `[ -f scripts/discover.sh ]`. No version checks, no capability negotiation.

**Why:** Discover is always installed as `scripts/discover.sh` by `discover init`. If it exists, it works. Adding version or capability checks creates coupling to discover's internals.

### Skills reference `--help`, not specific subcommands
Skill instructions tell the agent to use `bash scripts/discover.sh --help` to learn available subcommands rather than listing them inline.

**Why over hardcoded subcommand lists:** Discover's interface evolves. Hardcoded lists rot. `--help` is always current. This also keeps the skill instructions short.

**Alternative considered:** Embedding a subcommand reference table. Rejected because it couples skill updates to discover releases.

## Risks / Trade-offs

[Token cost of discover queries] → Each validation is a bash invocation. In explore sessions with many codebase references, this adds up. Mitigation: discover caches internally and queries are fast (<1s typically). The cost of a hallucinated reference is higher than the cost of a validation query.

[Duplication across three SKILL.md files] → The detection block and validation rule appear in three files. Mitigation: the duplicated content is ~10 lines. If discover's install path changes, three files need updating. Acceptable for now; `discover init` could generate the block in the future.

[Agent may over-validate] → The "all references are hallucinations" rule could cause the agent to validate obvious things (e.g., a file it just read exists). Mitigation: trust the agent's judgment on what constitutes a codebase reference vs. a file it's actively working with. The rule catches the important case: claims about relationships, definitions, and reachability.

## Open Questions

- Should `discover init` gain an `--openspec` flag that auto-injects the discover-awareness blocks into existing openspec skills? (Deferred to post-integration.)
