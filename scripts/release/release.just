# scripts/release/release.just — recetas de release. Importado desde el Justfile raíz.

# Prepara CHANGE.md, RELEASE.md, commit y tag anotado
# Uso: just tag-create v0.1.1
tag-create version:
    python3 scripts/release/tag-create.py {{version}}

# Sube commits a main y luego el tag — dispara el workflow de GitHub Actions
# Lee el tag activo desde RELEASE.md (escrito por tag-create)
tag-push:
    scripts/release/tag-push.sh

# Compila las librerías Rust (.so) y las publica en GHCR.
# Lee FFM_VERSION y JNI_VERSION desde el archivo VERSIONS.
# Debe ejecutarse ANTES de release-dispatch cuando las .so cambian.
#
# Uso:
#   just rust-publish
#   FFM_VERSION=0.2.0 just rust-publish    # override para solo FFM
rust-publish:
    scripts/release/rust-publish.sh

# Lanza release.yml manualmente vía workflow_dispatch con las versiones
# de librerías Rust definidas en el archivo VERSIONS.
# Alternativa a hacer push del tag v* cuando se necesita control explícito
# de qué versión de .so usar.
#
# Uso:
#   just release-dispatch v0.3.6
#   FFM_VERSION=0.2.0 just release-dispatch v0.3.6   # override de versión
#
# Flujo completo de un release:
#   just tag-create v0.3.6        → prepara CHANGELOG, commit y tag local
#   just rust-publish             → publica .so en GHCR (si cambiaron)
#   just tag-push                 → sube main + tag a GitHub
#   just release-dispatch v0.3.6  → lanza el pipeline de release con versiones de VERSIONS
release-dispatch version:
    scripts/release/release-dispatch.sh {{version}}
