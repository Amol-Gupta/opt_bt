<!-- 
SYNC IMPACT REPORT
Version: 0.0.0 -> 1.0.0
Title: Initial Constitution Ratification
Modified Principles:
- I. Performance-Critical Rust (New)
- II. Event-Driven Architecture (New)
- III. Testing Discipline (New)
- IV. Documentation Excellence (New)
- V. Simplicity & Modular Design (New)
- VI. Flexible Configuration (New)
Added Sections:
- Technical Standards
- Development Workflow
Templates requiring updates:
- .specify/templates/plan-template.md (⚠ pending)
- .specify/templates/spec-template.md (⚠ pending)
- .specify/templates/tasks-template.md (⚠ pending)
-->
# Opt_BT Constitution

## Core Principles

### I. Performance-Critical Rust
The application MUST be written in Rust to ensure memory safety without garbage collection performance penalties. Performance is a primary, non-negotiable requirement.
- **Target:** The backtester MUST be capable of processing a 3-year backtest in under 2 seconds.
- **Optimization:** Hot paths must be identified and optimized. Zero-cost abstractions should be preferred.
- **alloc:** Minimizing heap allocations in the hot path is strongly encouraged.

### II. Event-Driven Architecture
The system MUST follow an event-driven architecture to realistically simulate market behavior and enable modular strategy development.
- **Simulation:** The backtester processes time-ordered events (ticks, bars, signals) sequentially.
- **Decoupling:** Strategy logic MUST be decoupled from the execution engine via event interfaces.
- **Determinism:** The event loop must be deterministic to ensure identical results for identical data and parameters.

### III. Testing Discipline (NON-NEGOTIABLE)
Reliance on manual testing is strictly forbidden. A strong emphasis on automated testing is required to maintain system stability while optimizing for speed.
- **Unit Testing:** Comprehensive unit tests are mandatory for all traits, structs, and logic.
- **Coverage:** High test coverage is expected.
- **Benchmarks:** Performance benchmarks (using tools like `criterion`) are required for critical components to prevent regressions.

### IV. Documentation Excellence
Documentation is a first-class citizen and a required deliverable for every feature.
- **User Documentation:** Clear, accessible guides and API references (rustdoc) for end-users are mandatory.
- **Internal Documentation:** Complex logic MUST be explained with comments. Architectural decisions must be recorded.
- **Sync:** Code and documentation must never diverge. A PR without necessary doc updates is incomplete.

### V. Simplicity & Modular Design
Complexity is the enemy of both speed and reliability.
- **Modularity:** functionality should be broken down into small, reusable crates or modules.
- **Dependencies:** External dependencies should be scrutinized for performance impact before inclusion.
- **Clarity:** explicit control flow is preferred over implicit magic.

### VI. Flexible Configuration
The system MUST support multiple layers of configuration to serve both rapid experimentation and production pipelines.
- **Hierarchy:** Configuration must be loadable from (in priority order): CLI arguments > Environment Variables > JSON Configuration Files > Defaults.
- **Reproducibility:** The full configuration state used for a backtest must be serializable to allow exact reproduction of results.

## Technical Standards

**Language & Tooling:**
- **Rust:** Latest stable version.
- **Formatting:** `rustfmt` standard style is mandatory.
- **Linting:** Code must pass `clippy` checks. Warnings should be treated as errors unless explicitly justified.

**Performance Standards:**
- All core engine changes generally require benchmarking.
- Memory usage should be predictable.

## Development Workflow

**Contribution Process:**
1.  **Design:** Major changes start with a brief design proposal or issue.
2.  **Implementation:** Code is written in Rust, following the principles above.
3.  **Verification:** Verification involves both correctness (Unit Tests) and speed (Benchmarks).
4.  **Documentation:** Update README, Rustdocs, or internal guides.
5.  **Review:** Code review focuses on correctness, performance, and readability.

## Governance

This Constitution supersedes all other process documents.

**Amendment Process:**
- Updates to this document require a Pull Request with the "Governance" label.
- Major changes (Principles) require consensus from core maintainers.
- Versioning follows Semantic Versioning (MAJOR.MINOR.PATCH) as described in the Outline.

**Compliance:**
- All Pull Requests must be checked against these principles.
- Use `.specify/templates` for consistent artifact generation.

**Version**: 1.0.0 | **Ratified**: 2026-02-24 | **Last Amended**: 2026-02-24
