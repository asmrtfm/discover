## ADDED Requirements

### Requirement: Discover detection
The system SHALL detect whether `scripts/discover.sh` exists in the project root once per skill invocation. When absent, skills operate as before with no warning.

#### Scenario: Discover is present
- **WHEN** an openspec skill is invoked in a project with `scripts/discover.sh`
- **THEN** discover is available for validation queries

#### Scenario: Discover is absent
- **WHEN** an openspec skill is invoked in a project without `scripts/discover.sh`
- **THEN** the skill operates without discover validation

### Requirement: Codebase references are hallucinations until validated
When discover is available, any reference to the codebase — files, relationships, definitions, reachability — SHALL be considered a hallucination until validated via discover. This applies in all contexts: assertions, speculation, diagrams, brainstorming. The appropriate discover subcommand is determined by the agent via `discover --help`.

#### Scenario: Codebase reference made during any openspec operation
- **WHEN** an openspec skill is about to present a reference to the codebase
- **THEN** the reference MUST be validated via discover before being presented to the user

#### Scenario: Discover query fails
- **WHEN** a discover validation query exits non-zero
- **THEN** the skill SHALL report the failure transparently rather than falling back to grep and presenting unvalidated results as fact
