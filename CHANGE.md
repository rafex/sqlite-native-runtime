# Changelog

Todos los cambios notables de este proyecto se documentan aquí.
El formato sigue [Keep a Changelog](https://keepachangelog.com/es/1.1.0/).

## [Unreleased]

## [v0.1.4] — 2026-05-22

### ✨ Nuevas funcionalidades
- allow manual trigger of verify-release against latest published release

### 🐛 Correcciones
- register Panama FFM downcall stubs for GraalVM 25
- replace workflow_run with explicit dispatch to prevent spurious triggers
- correct verify-release workflow file issue

### 📝 Documentación
- document Raspberry Pi support — 64-bit OS required, arm32 not possible

## [v0.1.3] — 2026-05-22

### 📝 Documentación
- add installation guide, usage guide, and install.sh script

### 🧪 Tests
- add Dockerfile and suite for validating release artifacts

### ⚙️  CI / Build
- remove workflow_dispatch from verify-release — only triggers after Release
- use debian-slim, run install.sh from release, add verify-release workflow

## [v0.1.1] — 2026-05-22

### 🐛 Correcciones
- workflow_dispatch + RELEASE_TAG unificado + checkout con ref explícito

### 🔧 Mantenimiento
- sistema de release con CHANGE.md, RELEASE.md y scripts tag-create/tag-push

## [v0.1.0] — 2026-05-22

### ✨ Nuevas funcionalidades
- Binding Panama FFM (JEP 454) para libsqlite_native_runtime
- API SQLite completa: open/memory, prepare, step, bind, column, close
- Soporte WAL, savepoints, transacciones y recuperación de errores
- Cross-compilación Linux x86\_64 / arm64 con glibc 2.17+ via cargo-zigbuild
- GraalVM 25 Native Image compatible (--initialize-at-run-time)

### 🧪 Tests
- 137 unit tests Rust (TT-1)
- 50 FFI contract tests (TT-2)
- 158 Java unit tests con cobertura 99 % LINE (TT-3)
- 32 Java integration tests: concurrencia, WAL, datasets grandes (TT-3i)

### ⚙️  CI / Build
- GitHub Actions CI con matrix amd64/arm64 y path filters
- GitHub Actions Release: .so, thin JAR, fat JAR, Native Image y SHA256 por artefacto
- Makefile y Justfile delegados a scripts en scripts/*/
