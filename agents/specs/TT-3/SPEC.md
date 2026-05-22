# Spec: Tt 3

```toml
artifact_type = "spec"
id = "SPEC-TT-3"
state = "draft"
owner = "rafex"
initiative = "TT-3"
created_at = "2026-05-22"
updated_at = "2026-05-22"
related_tasks = []
related_decisions = []
artifacts = []
validation = []
```

## Problem

Los 128 tests unitarios Java verifican contratos de API en aislamiento (método a método, single-threaded, operaciones simples). No existe ningún test que verifique: concurrencia con virtual threads (Project Loom), múltiples SqliteConnection al mismo archivo con WAL, bulk inserts de 10k+ filas, blobs de 1MB+, recovery de errores en workflows reales, ni simulación de connection pool. El claim de thread-safety del código está sin validar empíricamente.

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
