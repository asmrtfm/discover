## ADDED Requirements

### Requirement: Proposals include validated impact evidence
When discover is available, the propose skill SHALL use discover to gather impact data for the Impact section rather than listing files found by grep.

#### Scenario: Impact section with discover available
- **WHEN** the propose skill generates an Impact section and discover is available
- **THEN** the impact data SHALL come from discover queries, not grep

#### Scenario: Discover failure during proposal
- **WHEN** a discover query fails during proposal generation
- **THEN** the propose skill notes the gap and proceeds — proposal generation MUST NOT fail because discover failed
