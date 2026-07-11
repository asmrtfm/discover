# discover.sh failure — 2026-07-09

scripts/discover.sh is the Kotlin variant (generated from kotlin.sh, LANG_ID="kotlin", scans src/main/kotlin/). This repo is TypeScript. The script was never regenerated for this project. All subcommands (live-files, usages, etc.) return empty results silently because the source directory doesn't exist.

Fix: regenerate from the TypeScript template (generate.sh typescript.sh --output discover.sh) and configure the entry point (App.tsx or index.js).

Task 4.2 (scan for stale imports) was completed with grep instead.
