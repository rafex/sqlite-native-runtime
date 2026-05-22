# Spec: Tt 4

```toml
artifact_type = "spec"
id = "SPEC-TT-4"
state = "draft"
owner = "rafex"
initiative = "TT-4"
created_at = "2026-05-22"
updated_at = "2026-05-22"
related_tasks = []
related_decisions = []
artifacts = []
validation = []
```

## Problem

El SmokeTest existe como clase Java manual pero no es automático ni está en CI. No hay verificación de que el binario nativo funciona tras cada cambio. GraalVM Native Image tiene comportamiento diferente a JVM: no hay class loading en runtime, AOT compilation, SymbolLookup.libraryLookup debe funcionar en runtime, SqliteLibrary debe inicializarse en runtime no en build time. Un regresión en el perfil native (ej. clase inicializada en build time por error) pasaría desapercibida.

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
