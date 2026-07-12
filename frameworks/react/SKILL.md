---
name: discover-react
description: React-specific structural analysis that complements discover-typescript. Uses ast-grep to map component hierarchies, custom hooks, context provider-consumer relationships, and route definitions.
---

## Why this exists

`discover.sh` does generic TypeScript/TSX static analysis — it resolves imports, finds definitions, and traces reachability. But it doesn't understand React-specific structure: which functions are components, where custom hooks are defined and used, how contexts wire providers to consumers, or how routes map paths to components. `discover-react.sh` fills that gap with React-aware AST queries.

Use `discover.sh` first for general import/definition questions. Escalate to `discover-react.sh` when the answer is React-specific.

## Script location

```
{{SCRIPT_PATH}}
```

Run from the repo root. Requires ast-grep and jq.

## Subcommands

### components

Find React components — functions (named or arrow) that return JSX.

```bash
bash {{SCRIPT_PATH}} components                    # list all components
bash {{SCRIPT_PATH}} components App                 # find a specific component
bash {{SCRIPT_PATH}} components src/pages/Home.tsx  # components in a file
bash {{SCRIPT_PATH}} components --json              # JSON output
```

### hooks

Find custom hook definitions (`useXxx` functions) across the project, or find all call sites of a specific hook.

```bash
bash {{SCRIPT_PATH}} hooks                    # list all custom hook definitions
bash {{SCRIPT_PATH}} hooks useAuth            # find all usages of useAuth
bash {{SCRIPT_PATH}} hooks useAuth --json     # JSON output
```

### contexts

Map `createContext` / `useContext` provider-consumer relationships. Shows where contexts are defined, which components provide them, and which components consume them.

```bash
bash {{SCRIPT_PATH}} contexts                     # full context map
bash {{SCRIPT_PATH}} contexts ThemeContext         # specific context
bash {{SCRIPT_PATH}} contexts --json              # JSON output
```

### routes

Extract route definitions from react-router `<Route>` elements.

```bash
bash {{SCRIPT_PATH}} routes              # list all routes
bash {{SCRIPT_PATH}} routes --json       # JSON output
```

## Rules

- Use `components` to understand the component hierarchy and find where components are defined.
- Use `hooks` without arguments to inventory custom hooks; with a hook name to find all consumers.
- Use `contexts` to trace data flow through React context — find the provider to understand what data a consumer receives.
- Use `routes` to map URL paths to components.
- When a component seems to receive data "magically," check `contexts` first, then `hooks`.
