## ADDED Requirements

### Requirement: Apply orients via discover before modifying code
When discover is available, the apply skill SHALL use discover to orient before modifying code for a task — locating the correct definition site, understanding what depends on the code being changed, and confirming the file's role in the dependency graph. This is proactive navigation, not after-the-fact validation.

#### Scenario: Task requires modifying existing code
- **WHEN** a task calls for changing existing code and discover is available
- **THEN** the apply skill SHALL use discover to locate and understand the target code before editing
