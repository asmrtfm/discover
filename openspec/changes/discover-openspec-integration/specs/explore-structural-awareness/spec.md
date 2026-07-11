## ADDED Requirements

### Requirement: Explore announces discover availability
The explore skill SHALL announce discover availability at session start so the user knows validated navigation is active.

#### Scenario: Discover available on explore entry
- **WHEN** the user enters explore mode in a project with discover available
- **THEN** the explore skill announces that codebase references will be validated via discover

### Requirement: Explore prefers discover over grep for codebase queries
When discover is available, the explore skill SHALL use discover subcommands rather than grep for questions about relationships, definitions, or reachability. Grep remains appropriate for text-level searches (string literals, comments, content within function bodies).

#### Scenario: Codebase question during exploration
- **WHEN** the user asks a question about how code connects, where something is defined, or what depends on what
- **THEN** the explore skill SHALL use discover rather than grep to answer
