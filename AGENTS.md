# Agent Instructions

## Project Overview

This project builds a SCADA application with a server-client architecture.

Primary documents:

- `docs/scada_basic_design.md`: Basic design and architecture source of truth.
- `docs/development_plan.md`: Development phases, deliverables, and completion criteria.
- `docs/design_review_findings.md`: Review findings and rationale behind important design decisions.

Agents must read the relevant design sections before making architecture or implementation changes.

## Core Architecture Principles

- Client applications must communicate with the server through REST API and MQTT over WebSocket.
- Communication drivers must not be called directly from clients or runtime UI.
- Tag values must flow through `Driver -> Driver Manager -> Tag Server -> Runtime -> Client`.
- Control commands must flow through `Client -> Runtime REST API -> Tag Server -> Driver Manager -> Driver`.
- Mock drivers must use the same path as production drivers.
- Runtime-direct mock shortcuts are not allowed.
- SVG assets must not contain tag definitions, control definitions, or modify rules.
- Screen definition JSON/DB is the source of truth for SVG bindings and modify rules.
- SurrealDB is used for internal data, design data, short-term state, and short-term cache.
- Long-term history, long-term audit logs, and large event history must be stored outside SurrealDB.
- Development can run services as Tauri child processes.
- Production should run server-side components as OS services, daemons, or container services.
- Individual communication drivers should run under Driver Manager rather than as standalone OS services.

## Planned Technology Direction

- Desktop shell: Tauri v2 + Rust.
- Frontend: Svelte + TypeScript.
- Internal process communication: gRPC.
- Client API: REST API.
- Realtime client updates: MQTT over WebSocket.
- Internal DB and local project data: SurrealDB.
- Long-term history: TimescaleDB, InfluxDB, or a dedicated history store.
- Service deployment: systemd, launchd, Windows Service, Docker Compose, or Kubernetes.

Do not replace these choices casually. If a change is necessary, update the design documents in the same change.

## Development Order

Prefer the order in `docs/development_plan.md`:

1. Contract definitions: DTOs, quality codes, ControlCommand states, gRPC, OpenAPI, JSON Schema.
2. Local service startup: Tauri Shell, SurrealDB, and service skeletons.
3. Mock Driver.
4. Driver Manager.
5. Tag Server.
6. Runtime.
7. Svelte monitoring screen.
8. Builder API.
9. Tag editor.
10. Screen editor.
11. ProjectVersion snapshot and preview reflection.
12. Service and daemon packaging.

## Coding Standards

- Keep responsibilities separated by component boundary.
- Keep Tauri Rust code thin: process management, OS integration, window management, file selection, and local service supervision.
- Put business logic in services such as Builder API, Tag Server, Runtime, Driver Manager, and Alarm Engine.
- Define external and internal contracts before relying on implicit data shapes.
- Use structured schemas for configuration data. Avoid ad hoc string parsing.
- Prefer typed DTOs for tag values, command states, alarm states, and driver messages.
- Use UTC for stored timestamps. Convert to user or equipment timezone at display boundaries.
- Keep comments concise and useful. Avoid comments that restate obvious code.
- Keep implementation changes scoped to the current phase.

## Contract And Data Rules

- gRPC contracts should be defined in `.proto` files.
- REST contracts should be defined in OpenAPI.
- Screen, tag, alarm, and driver manifest definitions should have JSON Schema.
- Tag value payloads must include enough information for ordering and quality handling:
  - `tag_id`
  - `value`
  - `data_type`
  - `quality`
  - `source_timestamp`
  - `server_timestamp`
  - `sequence`
- UI startup should use `REST snapshot + MQTT delta`.
- MQTT updates must be discarded if older than the current value by `sequence` or timestamp.
- Control commands must use explicit lifecycle states and preserve auditability.

## Security And Safety

- Never bypass Runtime, Tag Server, or Driver Manager boundaries for convenience.
- Do not send control writes through MQTT in the initial design.
- Sanitize imported SVGs. Remove scripts, external references, and event attributes.
- Local preview services should bind to `127.0.0.1` and require an ephemeral startup token.
- REST, MQTT, and gRPC authentication/authorization boundaries must be respected.
- Important operations should support reauthentication, confirmation, and audit logging.

## Testing Expectations

Add tests according to risk and phase. Important categories:

- gRPC contract tests.
- REST API contract tests.
- MQTT reconnect tests.
- Tag Server value, quality, and stale-state tests.
- ControlCommand lifecycle tests.
- SVG binding tests.
- ProjectVersion publish and rollback tests.
- SurrealDB migration tests.
- Local service startup and shutdown tests.

If tests cannot be run, document why in the final response.

## Documentation Policy

Update documentation when changing:

- Component responsibility.
- Data flow.
- Tag value model.
- Control command lifecycle.
- MQTT topic or payload.
- REST/gRPC contracts.
- Storage choice.
- Service/daemon deployment strategy.
- Development phase scope.

## Git Policy

- Keep the repository in a working state.
- Create commits as savepoints after meaningful design or implementation milestones.
- Prefer small, focused commits.
- Do not mix unrelated refactors with feature work.
- Before committing, check `git status --short`.

