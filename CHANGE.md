# Changelog

Todos los cambios notables de este proyecto se documentan aquí.
El formato sigue [Keep a Changelog](https://keepachangelog.com/es/1.1.0/).

## [Unreleased]

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
