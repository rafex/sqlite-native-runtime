#!/usr/bin/env bash
# tag-push — lee el tag activo de RELEASE.md, sube commits a main y luego el tag.
#
# Uso:   just tag-push
#        scripts/release/tag-push.sh
#
# Requisitos: haber ejecutado "just tag-create <version>" antes.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/../commons/env.sh"

RELEASE_MD="${ROOT}/RELEASE.md"

# ── Leer el tag desde RELEASE.md ─────────────────────────────────────────────
if [[ ! -f "$RELEASE_MD" ]]; then
  echo "Error: RELEASE.md no encontrado en ${ROOT}" >&2
  echo "       Ejecuta primero: just tag-create <version>" >&2
  exit 1
fi

TAG="$(grep -m1 '^<!-- RELEASE_TAG:' "$RELEASE_MD" \
  | sed 's/<!-- RELEASE_TAG:[[:space:]]*\(.*\)[[:space:]]*-->/\1/' \
  | tr -d '[:space:]')"

# ── Validar formato semver ────────────────────────────────────────────────────
if [[ -z "$TAG" ]] || ! [[ "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+ ]]; then
  echo "Error: RELEASE_TAG inválido o ausente en RELEASE.md ('${TAG}')" >&2
  echo "       Ejecuta: just tag-create <version>" >&2
  exit 1
fi

# ── Verificar que el tag existe localmente ────────────────────────────────────
if ! git -C "$ROOT" tag -l "$TAG" | grep -qx "$TAG"; then
  echo "Error: el tag '${TAG}' no existe localmente." >&2
  echo "       Ejecuta: just tag-create ${TAG}" >&2
  exit 1
fi

# ── Publicar ──────────────────────────────────────────────────────────────────
echo ""
echo "→ Publicando release ${TAG}"
echo ""

echo "  → git push origin main"
git -C "$ROOT" push origin main

echo "  → git push origin ${TAG}"
git -C "$ROOT" push origin "$TAG"

echo ""
echo "✅  Release ${TAG} enviado a GitHub."
echo "    El workflow release.yml generará los artefactos y creará el GitHub Release."
echo ""
