# Test Coverage — sqlite-native-runtime

## Resumen por capa

| Capa | Herramienta | Tests | Cobertura | Estado |
|---|---|---|---|---|
| 🦀 **TT-1** Rust Unit | `cargo test --lib` | **137** | **96.08% LINE** (llvm-cov) | ✅ Completo |
| 🔗 **TT-2** FFI Contract | `cargo test --test` | **50** | ABI surface cubierta | ✅ Completo |
| ☕ **TT-3** Java Unit | `mvn test` (JaCoCo) | **128** | **99% LINE** (JaCoCo) | ✅ Completo |
| ☕ **TT-3i** Java Integration | `mvn test -Pintegration` | **32** | escenarios reales | ✅ Completo |
| 🔮 **TT-4** GraalVM Native | `make native-test` | — | — | 🔲 Pendiente |
| 🐳 **Contenedores** | `make container-test` | **347** | TT-1+2 Rust · TT-3+3i Java | ✅ Completo |

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

Reporte HTML: `sources/java/ether-sqlite-runtime/target/site/jacoco/index.html`

### Distribución de tests

| Clase de test | Tests | Casos representativos |
|---|---|---|
| `SqliteExceptionTest` | 3 | constructor con mensaje, con causa, `instanceof RuntimeException` |
| `SqliteConnectionTest` | 61 | `open` (path, flags, memory, named), `@TempDir` con `toRealPath()` (NOFOLLOW macOS), WAL checkpoint, transacciones, savepoints, logging FINE, resource leak warning |
| `SqliteStatementTest` | 64 | `prepare`, todos los `bind*` (éxito + out-of-range + null), `step`, `columnBlob` vacío, `reset` con RAISE(ABORT), `close` con/sin callback, helpers estáticos |

---

## ☕ TT-3i — Java Integration Tests (`mvn test -Pintegration`)

Tests JUnit 5 con `@Tag("integration")` — escenarios realistas de uso que los unit tests
no pueden cubrir: concurrencia con virtual threads (Project Loom), múltiples conexiones
WAL, datasets grandes y recuperación de errores.

```
make test-integration   # o: mvn test -Pintegration
```

**Estado:** ✅ 32 tests, todos pasan.

### Distribución por clase `@Nested`

| Clase | Tests | Qué valida |
|---|---|---|
| `ConcurrentReadsTest` | 3 | 20/50 virtual threads leen la misma BD, columnas mixtas sin corrupción |
| `ConcurrentWritesTest` | 3 | 10×100 inserts, reads/writes interleaved, transacciones serializadas con lock |
| `MultiConnectionWalTest` | 4 | writer+reader WAL coexisten, readers concurrentes sin SQLITE_BUSY, checkpoint PASSIVE/TRUNCATE |
| `BulkInsertTest` | 3 | 10 000 inserts en una sola transacción, primera/última fila, named params |
| `LargeDataTest` | 4 | TEXT 1 MB, TEXT Unicode 1 MB, BLOB 1 MB round-trip, BLOB vacío ≠ NULL |
| `StatementReuseTest` | 3 | 1 000 ciclos insert/reset, 1 000 ciclos SELECT/reset, clearBindings entre ciclos |
| `ConnectionPoolSimTest` | 3 | 50 open/close secuenciales sin leak, 20 conexiones virtuales paralelas, 20 conexiones a fichero |
| `ErrorRecoveryTest` | 4 | rollback automático en exception, conexión reutilizable, withSavepoint scope, 5 fallos consecutivos |
| `SavepointNestingTest` | 3 | 3 niveles anidados commit, rollback inner preserva outer, semántica manual savepoint/release/rollbackTo |
| `WalCheckpointLoadTest` | 2 | auto-checkpoint desactivado + checkpoint manual, dbName null vs "" equivalentes |

### Notas de diseño

- `SqliteStatement` **no es thread-safe** — cada virtual thread crea su propia instancia.
- `SqliteConnection` es thread-safe (FULLMUTEX + Mutex Rust), pero `BEGIN/COMMIT`
  no se puede interleave: se usa `synchronized(db)` para serializar transacciones concurrentes.
- `@TempDir` resuelto con `toRealPath()` — evita el symlink `/var/folders` → `/private/var/folders`
  en macOS con `SQLITE_OPEN_NOFOLLOW`.

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
| Java unit (TT-3) | **128** | ≥ 99% LINE Java |
| Java integration (TT-3i) | **32** | escenarios reales |
| **Grand total** | **347** | — |

---

## 🐳 Contenedores (Podman / Docker)

Tests en contenedor Linux (Debian 12 slim) — aíslan el entorno del host y se
reutilizan en CI (GitHub Actions). Compatible con Podman (local) y Docker (CI).

```
make container-test-rust    # TT-1+TT-2 en contenedor (Debian 12 + Rust stable)
make container-test-java    # TT-3+TT-3i en contenedor (multi-stage)
make container-test         # ambos en secuencia
```

Para usar Docker en lugar de Podman:
```bash
make container-test CONTAINER_ENGINE=docker
```

### Diseño de las imágenes

| Imagen | Base | Etapas | Ejecuta |
|---|---|---|---|
| `snr-rust-test` | `debian:12-slim` | 1 | `cargo test --lib` + `cargo test --test ffi_contract` |
| `snr-java-test` | `debian:12-slim` → `eclipse-temurin:24-jdk-noble` | 2 | `mvn test` + `mvn test -Pintegration` |

La etapa 1 de `snr-java-test` compila el `.so` nativo dentro del contenedor —
no requiere tener Rust instalado en el host para correr los tests Java en contenedor.

SQLite está **bundled** en la crate (`features = ["bundled"]`): no requiere
`libsqlite3-dev` en el sistema. Solo necesita `build-essential` (gcc).

---

## Ejecutar todo

```bash
# Host macOS (requiere GraalVM 25 + Rust instalados)
make test-rust          # 137 Rust unit tests
make test-ffi           # 50 FFI contract tests
make coverage-rust      # cobertura Rust (requiere cargo-llvm-cov)
make test-unit          # 128 Java unit tests
make test-integration   # 32 Java integration tests
make coverage           # 128 tests + reporte JaCoCo HTML

# Contenedor Linux (solo requiere Podman o Docker)
make container-test-rust   # TT-1 + TT-2 en contenedor
make container-test-java   # TT-3 + TT-3i en contenedor
make container-test        # todos los anteriores
```
