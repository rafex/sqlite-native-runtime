# Arquitectura — sqlite-native-runtime

## Descripción

Binding Java → Rust → SQLite usando Panama FFM (Java 22+, JEP 454) y GraalVM 25.
Tres capas: librería nativa Rust (`cdylib`/`staticlib`), binding Panama FFM (`SqliteLibrary`),
y API de alto nivel (`SqliteConnection`, `SqliteStatement`).

## Stack técnico

- **Rust** 1.x stable, edition 2021, `libsqlite3-sys 0.30` con feature `bundled`
- **Java** 22+ (target bytecode 22), compilado con GraalVM JDK 25.0.2
- **Build** Cargo (Rust) + Maven Wrapper 3.9.9 (Java) + Makefile / Justfile
- **Cross-compilation** `cargo-zigbuild` para Linux x86_64 y arm64 (glibc 2.17+)
- **Native Image** GraalVM Build Tools `native-maven-plugin 0.10.6`

## Estructura de directorios

```
sqlite-native-runtime/
├── sqlite-native-runtime/
│   ├── rust/              Crate Rust (cdylib + staticlib)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── connection.rs    snr_open*, snr_close, snr_exec, ...
│   │       ├── statement.rs     snr_prepare, snr_bind_*, snr_column_*, snr_step
│   │       ├── transaction.rs   snr_begin*, snr_commit, snr_rollback, snr_savepoint*
│   │       ├── wal.rs           snr_wal_checkpoint, snr_wal_autocheckpoint
│   │       ├── error.rs         thread-local LAST_ERROR, snr_last_error*
│   │       ├── handle.rs        Handle (Arc<Mutex<RawConn>>), RawConn::Drop
│   │       ├── stmt.rs          StmtHandle
│   │       └── util.rs          cstr_to_str
│   └── java/              Maven project
│       └── src/
│           ├── main/java/mx/rafex/sqlite/
│           │   ├── SqliteLibrary.java    Panama FFM downcalls (excluido de JaCoCo)
│           │   ├── SqliteConnection.java API alto nivel, AutoCloseable
│           │   ├── SqliteStatement.java  API alto nivel, AutoCloseable
│           │   └── SqliteException.java  RuntimeException
│           └── test/java/mx/rafex/sqlite/
│               ├── SqliteExceptionTest.java
│               ├── SqliteConnectionTest.java
│               ├── SqliteStatementTest.java
│               └── SmokeTest.java        test manual / base para native image
├── docs/testing/          Estrategia de tests (ver strategy.md)
├── agents/                SpecNative: contexto, arquitectura, iniciativas
├── Makefile               Build orquestador
└── PLAN.md                Histórico de decisiones y progreso
```

## Flujo de memoria (ABI)

```
Java String  →  Arena.ofConfined().allocateFrom(s)  →  *const c_char  →  Rust CStr
Rust *mut c_char (heap Rust)  →  MemorySegment (Java)  →  readAndFreeString  →  snr_free_string
Rust *mut Handle  →  MemorySegment (Java)  →  opaque handle (no liberar directamente)
```

## Invariantes críticos

1. `SQLITE_OPEN_FULLMUTEX` forzado siempre en `snr_open` y `snr_open_memory`
2. `SQLITE_OPEN_NOFOLLOW` forzado siempre (rechaza symlinks)
3. `panic = "abort"` en todos los perfiles Rust (evita unwinding a través de FFI)
4. `snr_last_error_copy()` para Project Loom — `snr_last_error()` sólo en single-thread
5. `checkOpen()` antes de toda operación en SqliteConnection y SqliteStatement

## Decisiones clave

- `DEC-0001` `libsqlite3-sys` bundled (no rusqlite, no sistema SQLite)
- `DEC-0002` Panama FFM no JNI (GraalVM Native Image compatible)
- `DEC-0003` `sqlite3_close` no `sqlite3_close_v2` (Arc garantiza orden de destrucción)
- `DEC-0005` Sin límite de longitud SQL (librería interna, no de red)
- `DEC-TEST-1..4` Ver docs/testing/strategy.md
