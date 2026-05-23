# scripts/release/release.just — recetas de release. Importado desde el Justfile raíz.

# Prepara CHANGE.md, RELEASE.md, commit y tag anotado
# Uso: just tag-create v0.1.1
tag-create version:
    python3 scripts/release/tag-create.py {{version}}

# Sube commits a main y luego el tag — dispara el workflow de GitHub Actions
# Lee el tag activo desde RELEASE.md (escrito por tag-create)
tag-push:
    scripts/release/tag-push.sh
