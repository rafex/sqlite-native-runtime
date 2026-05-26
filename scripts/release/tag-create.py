#!/usr/bin/env python3
"""
tag-create — prepara CHANGE.md, RELEASE.md, hace commit y crea el tag anotado.

Uso:
    just tag-create v0.1.1
    python3 scripts/release/tag-create.py v0.1.1

Flujo:
    1. Valida que la versión sea semver (vMAJOR.MINOR.PATCH[-sufijo])
    2. Verifica que el tag no exista y que el worktree esté limpio
    3. Recoge los commits desde el último tag (o todos si es el primero)
    4. Clasifica los commits por tipo (conventional commits)
    5. Escribe RELEASE.md  (release actual, leído por tag-push y por release.yml)
    6. Prepend en CHANGE.md (historial acumulado de todas las versiones)
    7. git add + git commit "chore(release): vX.Y.Z"
    8. git tag -a  vX.Y.Z  (tag anotado con el cuerpo del release como mensaje)
"""

from __future__ import annotations

import re
import subprocess
import sys
from datetime import date
from pathlib import Path

# ── Rutas ─────────────────────────────────────────────────────────────────────
ROOT = Path(__file__).resolve().parent.parent.parent
CHANGE_MD = ROOT / "CHANGE.md"
RELEASE_MD = ROOT / "RELEASE.md"

# ── Validación de versión ─────────────────────────────────────────────────────
SEMVER_RE = re.compile(r"^v\d+\.\d+\.\d+([.\-][a-zA-Z0-9.]+)?$")

# ── Mapa conventional commits → sección del changelog ────────────────────────
# El orden de este dict define el orden de aparición en el changelog.
SECTIONS: dict[str, str] = {
    "feat":     "### ✨ Nuevas funcionalidades",
    "fix":      "### 🐛 Correcciones",
    "perf":     "### ⚡ Rendimiento",
    "refactor": "### ♻️  Refactors",
    "docs":     "### 📝 Documentación",
    "test":     "### 🧪 Tests",
    "ci":       "### ⚙️  CI / Build",
    "build":    "### ⚙️  CI / Build",
    "chore":    "### 🔧 Mantenimiento",
}
OTHER_SECTION = "### 🔀 Otros cambios"


# ── Helpers git ───────────────────────────────────────────────────────────────

def _run(cmd: list[str], check: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT, check=check)


def last_tag() -> str | None:
    r = _run(["git", "describe", "--tags", "--abbrev=0"], check=False)
    return r.stdout.strip() if r.returncode == 0 else None


def tag_exists(version: str) -> bool:
    return bool(_run(["git", "tag", "-l", version]).stdout.strip())


def worktree_clean() -> bool:
    """True si no hay cambios staged ni unstaged (archivos nuevos sin track se ignoran)."""
    r = _run(["git", "status", "--porcelain"])
    # Filtra líneas de archivos sin track (??)
    dirty = [l for l in r.stdout.splitlines() if not l.startswith("??")]
    return len(dirty) == 0


def git_log_since(tag: str | None) -> list[str]:
    if tag:
        cmd = ["git", "log", f"{tag}..HEAD", "--oneline", "--no-merges"]
    else:
        cmd = ["git", "log", "--oneline", "--no-merges"]
    r = _run(cmd)
    return [l for l in r.stdout.splitlines() if l.strip()]


# ── Clasificación de commits ──────────────────────────────────────────────────

# Regex para conventional commits: <hash> <tipo>[(<scope>)][!]: <descripción>
_CONV_RE = re.compile(r"^[0-9a-f]+ ([a-z]+)(?:\([^)]*\))?!?:\s*(.+)$")


def classify(lines: list[str]) -> dict[str, list[str]]:
    """Agrupa mensajes de commit por sección del changelog."""
    buckets: dict[str, list[str]] = {}
    for line in lines:
        m = _CONV_RE.match(line)
        if m:
            kind, desc = m.group(1), m.group(2).strip()
            section = SECTIONS.get(kind, OTHER_SECTION)
        else:
            # No es conventional commit → limpia el hash y va a "Otros"
            desc = re.sub(r"^[0-9a-f]+\s+", "", line).strip()
            section = OTHER_SECTION
        buckets.setdefault(section, []).append(f"- {desc}")
    return buckets


# ── Generación de texto ───────────────────────────────────────────────────────

def build_body(version: str, buckets: dict[str, list[str]], today: str) -> str:
    """Devuelve el cuerpo del release (sin la línea de tag de RELEASE.md)."""
    lines = [f"# Release {version} — {today}", ""]

    # Orden: según SECTIONS (preserva inserción), luego OTHER al final
    order: list[str] = list(dict.fromkeys(list(SECTIONS.values()) + [OTHER_SECTION]))

    for section in order:
        if section in buckets:
            lines.append(section)
            lines.extend(buckets[section])
            lines.append("")

    if not buckets:
        lines += ["_Sin cambios registrados desde el release anterior._", ""]

    return "\n".join(lines)


# ── Escritura de ficheros ─────────────────────────────────────────────────────

def write_release_md(version: str, body: str) -> None:
    """RELEASE.md: primera línea = tag machine-readable, resto = body del release."""
    RELEASE_MD.write_text(f"<!-- RELEASE_TAG: {version} -->\n{body}", encoding="utf-8")
    print(f"  ✓ RELEASE.md  ← {version}")


def prepend_change_md(version: str, body: str, today: str) -> None:
    """Inserta la entrada de la nueva versión en CHANGE.md justo tras [Unreleased]."""
    # Cuerpo sin el título H1 (que ya proporciona el encabezado de CHANGE.md)
    body_lines = [
        l for l in body.splitlines()
        if not l.startswith("# Release")
    ]
    entry = f"## [{version}] — {today}\n\n" + "\n".join(body_lines).lstrip("\n") + "\n"

    if CHANGE_MD.exists():
        old = CHANGE_MD.read_text(encoding="utf-8")
        marker = "## [Unreleased]\n"
        if marker in old:
            new = old.replace(marker, f"{marker}\n{entry}", 1)
        else:
            # Sin sección Unreleased: insertar antes del primer ## [vX]
            new = re.sub(r"(\n)(## \[v)", rf"\n{entry}\n\2", old, count=1)
            if new == old:
                new = old.rstrip("\n") + f"\n\n{entry}"
    else:
        new = (
            "# Changelog\n\n"
            "Todos los cambios notables de este proyecto se documentan aquí.\n"
            "El formato sigue [Keep a Changelog](https://keepachangelog.com/es/1.1.0/).\n\n"
            "## [Unreleased]\n\n"
            f"{entry}"
        )

    CHANGE_MD.write_text(new, encoding="utf-8")
    print(f"  ✓ CHANGE.md   ← {version}")


# ── Git commit y tag ──────────────────────────────────────────────────────────

def git_commit_and_tag(version: str, body: str) -> None:
    _run(["git", "add", str(CHANGE_MD), str(RELEASE_MD)])
    _run(["git", "commit", "-m", f"chore(package): {version}"])
    print(f"  ✓ commit      ← chore(package): {version}")

    # Tag anotado: el mensaje es el cuerpo del release (multilinea)
    _run(["git", "tag", "-a", version, "-m", body])
    print(f"  ✓ tag         ← {version} (anotado)")


# ── Main ──────────────────────────────────────────────────────────────────────

def main() -> None:
    if len(sys.argv) < 2:
        sys.exit("Uso: tag-create.py <version>  ej: v0.1.1  v2.0.0-beta.1")

    version: str = sys.argv[1]
    if not version.startswith("v"):
        version = f"v{version}"

    if not SEMVER_RE.match(version):
        sys.exit(
            f"Error: '{version}' no es semver válido.\n"
            "  Formatos aceptados: v0.1.0  v2.0.0  v1.2.3-beta.1  v10.0.0-rc.2"
        )

    if tag_exists(version):
        sys.exit(f"Error: el tag '{version}' ya existe.")

    if not worktree_clean():
        sys.exit(
            "Error: hay cambios sin commitear en el worktree.\n"
            "  Haz commit o stash antes de crear el release."
        )

    today = date.today().isoformat()
    prev = last_tag()

    print(f"\n→ Preparando release {version}")
    print(f"  Tag anterior : {prev or '(ninguno — primer release)'}")

    commits = git_log_since(prev)
    print(f"  Commits      : {len(commits)}")

    buckets = classify(commits)
    body = build_body(version, buckets, today)

    write_release_md(version, body)
    prepend_change_md(version, body, today)
    git_commit_and_tag(version, body)

    print(f"\n✅  Release {version} preparado localmente.")
    print(f"    Para publicar en GitHub:  just tag-push\n")


if __name__ == "__main__":
    main()
