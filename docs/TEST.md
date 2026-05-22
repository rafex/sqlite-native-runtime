# Test Coverage — sqlite-native-runtime

## Resumen por capa

| Capa | Herramienta | Tests | Cobertura | Estado |
|---|---|---|---|---|
| 🦀 **TT-1** Rust Unit | `cargo test --lib` | **137** | **96.08% LINE** (llvm-cov) | ✅ Completo |
| 🔗 **TT-2** FFI Contract | `cargo test --test` | **50** | ABI surface cubierta | ✅ Completo |
| ☕ **TT-3** Java Unit | `mvn test` (JaCoCo) | **128** | **99% LINE** (JaCoCo) | ✅ Completo |
| ☕ **TT-3i** Java Integration | `@Tag("integration")` | — | — | 🔲 Pendiente |
| 🔮 **TT-4** GraalVM Native | `make native-test` | — | — | 🔲 Pendiente |

---

## 🦀 TT-1 — Rust Unit Tests (`cargo test --lib`)

Tests `#[cfg(test)]` en los 8 módulos fuente de la crate. Acceden a helpers
`pub(crate)` internos — cubren lógica no visible desde la ABI C.

```
make test-rust
```

### Cobertura llvm-cov (cargo-llvm-cov)

| Módulo | Líneas cubiertas | Regiones | Funciones |
|---|---|---|---|
| `error.rs` | **100.00%** | 100.00% | 100% |
| `util.rs` | **100.00%** | 100.00% | 100% |
| `stmt.rs` | **100.00%** | 100.00% | 100% |
| `transaction.rs` | **98.54%** | 98.96% | 100% |
| `wal.rs` | **97.14%** | 96.80% | 100% |
| `statement.rs` | **96.43%** | 96.61% | 98.8% |
| `connection.rs` | **93.17%** | 94.23% | 100% |
| `handle.rs` | **82.98%** | 97.18% | 100% |
| **TOTAL** | **96.08%** | **96.81%** | **99.53%** |

### Distribución de tests por módulo

| Módulo | Tests | Qué cubre |
|---|---|---|
| `error.rs` | 10 | `thread_local` LAST_ERROR, copia heap, truncado NUL interior, aislamiento por hilo, free-null noop |
| `util.rs` | 4 | `cstr_to_str`: NULL, UTF-8 válido/vacío/inválido |
| `handle.rs` | 4 | `handle_ref` null/válido, `Drop` cierra sqlite3, Arc refcount |
| `stmt.rs` | 3 | `stmt_ref` null/válido, Arc compartido con conexión |
| `connection.rs` | 26 | `snr_open_memory`, `snr_open` (file/flags/errores), ping, versión, exec, rowid, changes, busy_timeout, constantes |
| `statement.rs` | 55 | prepare, step (ROW/DONE/error), reset, clear_bindings, bind_{null,int,double,text,blob} + out-of-range + len negativo, parameter_index, todas las column_*, constantes |
| `transaction.rs` | 19 | begin/immediate/exclusive/commit/rollback, double-begin, sin transacción activa, savepoint/release/rollback_to + escape de `"` + anidados |
| `wal.rs` | 16 | checkpoint en :memory:, out-params, handle nulo, DB inexistente, string vacío, autocheckpoint, constantes de modo |

### Líneas no cubiertas (61 de 1 557)

Todas son ramas defensivas de fault-injection genuinamente no testeables sin mocking del runtime nativo:

- **`handle.rs` (8)** — `eprintln!` en `RawConn::Drop` cuando `sqlite3_close` devuelve `SQLITE_BUSY`.
- **`connection.rs` (22)** — rutas donde `sqlite3_open_v2` falla con `db != NULL` (requiere OOM del SO) y mutex envenenado.
- **`statement.rs` (24)** — mutex envenenado en downcalls; recuperación del poison en `snr_stmt_close`.
- **`transaction.rs` (3)** / **`wal.rs` (4)** — mutex envenenado y error de `sqlite3_wal_autocheckpoint` en conexión válida.

---

## 🔗 TT-2 — FFI Contract Tests (`cargo test --test`)

Tests en `tests/ffi_contract.rs` — crate externo que **solo ve símbolos `pub`**,
exactamente igual que Java via Panama FFM. No puede llamar helpers internos.

```
make test-ffi
```

### Distribución por categoría

| Categoría | Tests | Qué valida |
|---|---|---|
| `null_safety` | 17 | Toda función con argumento nulo → código de error correcto, sin crash ni UB |
| `memory_ownership` | 6 | Protocolo free/no-free de cada puntero devuelto por la ABI |
| `error_propagation` | 8 | Estado de error tras fallos, limpieza en éxito, contenido de `copy == internal`, A-3 / A-2 |
| `thread_isolation` | 3 | `thread_local` aislado entre OS threads; copia heap sobrevive al clear |
| `lifecycle` | 9 | Secuencias ABI completas: CRUD, named params, reset ×5, commit/rollback, savepoint parcial, file DB |
| `abi_flags` | 7 | Valores exactos: SNR_ROW=1, SNR_DONE=0, SNR_ERROR=-1; INTEGER=1…NULL=5; checkpoint modes |
| `concurrent` | 2 | 4 hilos con `:memory:` independientes; 2 hilos con named memory DB (shared-cache) |

### Cambios de soporte

- `Cargo.toml`: `crate-type` incluye `"rlib"` — necesario para que `cargo test --test` linkee el binary.
- `lib.rs`: re-exporta `Handle` y `StmtHandle` — aparecen en firmas de `snr_open`/`snr_prepare` y deben ser nombables por crates externos.

---

## ☕ TT-3 — Java Unit Tests (`mvn test`, JaCoCo)

Tests JUnit 5 sobre la API Java de alto nivel (`SqliteConnection`, `SqliteStatement`,
`SqliteException`). Requieren la `.dylib` compilada.

```
make test-unit    # ejecuta tests
make coverage     # tests + reporte JaCoCo HTML
```

### Cobertura JaCoCo

| Clase | Tests | Cobertura LINE |
|---|---|---|
| `SqliteException` | 3 | 100% |
| `SqliteConnection` | 61 | ~99% |
| `SqliteStatement` | 64 | ~99% |
| `SqliteLibrary` | — | excluida (block `static` con fallbacks de filesystem) |
| **TOTAL** | **128** | **≥ 99%** |

Umbral configurado: `LINE ≥ 0.99` (no 1.00 — 2 líneas genuinamente no testeables
documentadas en `pom.xml`).

Reporte HTML: `sqlite-native-runtime/java/target/site/jacoco/index.html`

### Distribución de tests

| Clase de test | Tests | Casos representativos |
|---|---|---|
| `SqliteExceptionTest` | 3 | constructor con mensaje, con causa, `instanceof RuntimeException` |
| `SqliteConnectionTest` | 61 | `open` (path, flags, memory, named), `@TempDir` con `toRealPath()` (NOFOLLOW macOS), WAL checkpoint, transacciones, savepoints, logging FINE, resource leak warning |
| `SqliteStatementTest` | 64 | `prepare`, todos los `bind*` (éxito + out-of-range + null), `step`, `columnBlob` vacío, `reset` con RAISE(ABORT), `close` con/sin callback, helpers estáticos |

---

## 🔮 TT-4 — GraalVM Native (`make native-test`)

Valida que la librería funciona compilada como ejecutable nativo GraalVM (AOT).
Detecta: clases no registradas en la reflexión, proxies faltantes, inicialización
estática no compatible con AOT.

```
make native    # compila SmokeTest como binario nativo
```

**Estado:** 🔲 Pendiente de automatización.

El `SmokeTest.java` manual ya existe y puede ejecutarse tras `make native`.
La automatización (`make native-test` que falle el build si el binario retorna
código ≠ 0) está planificada en TT-4 del backlog.

---

## Acumulado total

| Scope | Tests | Cobertura |
|---|---|---|
| Rust (TT-1 + TT-2) | **187** | 96.08% LINE Rust |
| Java (TT-3 unit) | **128** | ≥ 99% LINE Java |
| **Grand total** | **315** | — |

---

## Ejecutar todo

```bash
# Capa Rust completa
make test-rust   # 137 unit tests
make test-ffi    # 50 FFI contract tests

# Cobertura Rust (requiere: cargo install cargo-llvm-cov)
make coverage-rust

# Java
make test-unit   # 128 JUnit tests
make coverage    # 128 tests + reporte JaCoCo
```
