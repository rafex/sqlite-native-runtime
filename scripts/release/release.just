# scripts/release/release.just — recetas de release. Importado desde el Justfile raíz.
#
# Flujos de automatización:
#
#   Flujo 1 — Validación en main (automático):
#     Se dispara al hacer push a main con cambios en sources/.
#     Solo compila y ejecuta tests. Nunca publica ni crea releases.
#
#   Flujo 2 — Publicación por tag (automático al hacer just tag-push):
#     Al hacer push de un tag v*, compile.yml compila todo y publica
#     los artefactos en GitHub Packages (GHCR).
#
#   Flujo 3 — Release final (manual):
#     just release v1.0.0
#     Lee VERSIONS, descarga de GHCR y crea el GitHub Release.

# ─────────────────────────────────────────────────────────────────────────────
# Prepara CHANGE.md, RELEASE.md, commit y tag anotado
# Uso: just tag-create v1.0.0
tag-create version:
    python3 scripts/release/tag-create.py {{version}}

# Sube commits a main y luego el tag — dispara Flujo 2 (publish.yml en CI)
# Lee el tag activo desde RELEASE.md (escrito por tag-create)
tag-push:
    scripts/release/tag-push.sh

# ─────────────────────────────────────────────────────────────────────────────
# Flujo 3: Construye el release final.
#
# Lee el archivo VERSIONS:
#   RUST=v1.0.0        → librerías .so Rust en GHCR
#   JAVA_FAT=v1.0.0    → JARs thin+fat en GHCR
#   JAVA_NATIVE=v1.0.0 → binarios GraalVM native en GHCR
#
# Descarga todos los artefactos desde GHCR y crea el GitHub Release.
#
# Uso:
#   just release v1.0.0
#
# Prerequisitos:
#   1. El tag v1.0.0 debe existir en el remoto (just tag-push)
#   2. Los paquetes de VERSIONS deben estar en GHCR (Flujo 2 completado)
#   3. gh CLI autenticado
release version:
    scripts/release/release.sh {{version}}

# ─────────────────────────────────────────────────────────────────────────────
# Publicación manual de librerías Rust independientes.
#
# Usa cuando solo cambia el código Rust (sin cambios Java).
# Lee RUST= de VERSIONS y dispara build-rust.yml con esa versión
# (tanto FFM como JNI al mismo tiempo).
#
# Uso:
#   just rust-publish
rust-publish:
    scripts/release/rust-publish.sh
