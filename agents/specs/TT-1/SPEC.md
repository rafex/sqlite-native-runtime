# Spec: Tt 1

```toml
artifact_type = "spec"
id = "SPEC-TT-1"
state = "draft"
owner = "rafex"
initiative = "TT-1"
created_at = "2026-05-22"
updated_at = "2026-05-22"
related_tasks = []
related_decisions = []
artifacts = []
validation = []
```

## Problem

El crate Rust tiene 0 tests. Toda la lógica de gestión de memoria (Arc/Mutex/Drop), propagación de errores (thread-local LAST_ERROR), validación de punteros nulos y manejo de tipos SQLite está sin cobertura automatizada. Un bug en esta capa (ej. mutex no adquirido, null check incorrecto) es invisible desde los tests Java porque Java solo ve el -1 / NULL de retorno.

## Goal

<!-- What must be true when this spec is done? -->

## Acceptance Criteria

- [ ] <!-- criterion 1 -->
- [ ] <!-- criterion 2 -->

## Out of Scope

<!-- What is explicitly NOT part of this spec? -->

## Related Decisions

<!-- DEC-XXXX: Short title -->

## Risks

<!-- Identified risks and mitigations -->

## Notes

<!-- Any context, constraints, or open questions -->
