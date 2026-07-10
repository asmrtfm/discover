---
name: usage-guides
description: "Create language-specific ast-grep usage guides for the discover framework. Spins up tasks for each language and works through them sequentially."
when_to_use: "When the user says anything like: 'write usage guide for <lang>', 'add usage guide', 'create usage docs for <lang>', or after adding a new language to discover and needing its usage guide."
---

## Arguments

Space-separated language names. Each must have a lang config at `langs/<lang>.sh`.

```
/usage-guides dart swift
/usage-guides crystal
```

If no arguments, find lang configs that are missing a corresponding `<lang>-usage.md` and offer those.

## Steps

1. For each language argument, confirm `langs/<lang>.sh` exists. Skip any that already have `langs/<lang>-usage.md`.

2. Create one task per language using the template in `tasks-template.json` (in this skill's directory), substituting `{{lang}}` and `{{LANG_NAME}}`.

3. Read the handoff at `HANDOFF.md` (in this skill's directory). It is the complete specification — what each guide covers, where the source information lives, and the process.

4. Work one language at a time, sequentially. User reviews each before moving to the next.

5. For each language, read these files before writing the guide:
   - `langs/<lang>.sh`
   - `crates/language/src/lib.rs` (to check which macro the language uses)
   - `crates/language/src/<lang>.rs` (if it exists — custom impl)
   - `tests/fixtures/<lang>.<ext>`
   - `tests/test-<lang>.sh`
   - One completed guide (e.g. `langs/dart-usage.md`) for tone and structure

6. Write the guide to `langs/<lang>-usage.md`. Mark the task complete.
