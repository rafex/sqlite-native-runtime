# Spec: Tt 2

```toml
artifact_type = "spec"
id = "SPEC-TT-2"
state = "draft"
owner = "rafex"
initiative = "TT-2"
created_at = "2026-05-22"
updated_at = "2026-05-22"
related_tasks = []
related_decisions = []
artifacts = []
validation = []
```

## Problem

Los tests unitarios Rust (TT-1) pueden llamar funciones pub(crate) internas. El contrato C ABI — lo que Java verá exactamente via Panama FFM — no está validado como unidad separada. Falta verificar: que #[no_mangle] exporta los símbolos correctos, que el ownership de memoria (snr_free_string, punteros internos vs transferidos) se cumple, que null en cualquier argumento no causa segfault, y que el thread-local de error está aislado entre hilos.

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
