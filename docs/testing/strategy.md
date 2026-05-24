# Estrategia de Testing — sqlite-native-runtime

**Última actualización:** 2026-05-22
**Estado:** Análisis completado. Java unit tests (128 tests, 99% LINE) implementados.
Resto pendiente — ver iniciativas TT-1..TT-4.

---

## 1. Mapa de capas y sus riesgos

El proyecto tiene **cuatro capas de código** con perfiles de riesgo distintos:

```
┌───────────────────────────────────────────────────────────┐
│  Java app / usuario final                                 │
├───────────────────────────────────────────────────────────┤
│  SqliteConnection / SqliteStatement  (Java alto nivel)    │  ← unit tests ✅ (128 tests)
├───────────────────────────────────────────────────────────┤
│  SqliteLibrary (Panama FFM downcalls)                     │  ← excluido de coverage
├══════════════════════════════════════════════════════════ ═╡
│  BOUNDARY FFI  C ABI extern "C"                           │  ← TT-2 FFI contract
├───────────────────────────────────────────────────────────┤
│  Rust: connection / statement / transaction / wal / error │  ← TT-1 Rust unit tests
├───────────────────────────────────────────────────────────┤
│  libsqlite3 (bundled via libsqlite3-sys)                  │  (no testeamos SQLite)
└───────────────────────────────────────────────────────────┘

   + TT-3 Java integration  →  stack completo, escenarios reales
   + TT-4 GraalVM native    →  compilación nativa, SmokeTest automatizado
```

Cada capa puede introducir defectos que las capas superiores no detectan:

| Capa | Defectos típicos | Test que lo detecta |
|------|-----------------|---------------------|
| Rust (lógica) | Null check incorrecto, mutex no adquirido, error no propagado | TT-1 Rust unit |
| Rust (ABI C) | Símbolo no exportado, firma incorrecta, ownership mal traspasado | TT-2 FFI contract |
| Panama FFM | FunctionDescriptor erróneo, Arena leak, MemorySegment size incorrecto | Java unit tests ✅ |
| Java alto nivel | Flujo de error no cubierto, recurso no cerrado | Java unit tests ✅ |
| Escenarios reales | Concurrencia, WAL bajo carga, grandes datasets | TT-3 Java integration |
| Native Image | Clase inicializada en build time, `SymbolLookup` en AOT | TT-4 GraalVM native |

---

## 2. Qué existe hoy

| Capa | Herramienta | Tests | Cobertura | Estado |
|------|------------|-------|-----------|--------|
| Java unit | JUnit 5 + JaCoCo | 128 | 99% LINE (excl. SqliteLibrary) | ✅ commit `ae951c7` |
| Rust | — | 0 | 0% | ❌ pendiente TT-1 |
| FFI contract | — | 0 | — | ❌ pendiente TT-2 |
| Java integration | — | 0 | — | ❌ pendiente TT-3 |
| GraalVM native | SmokeTest manual | manual | — | ⚠️ manual, pendiente TT-4 |

---

## 3. TT-1 — Rust Unit Tests

### Objetivo
Verificar la lógica interna de Rust en aislamiento sin cruzar la frontera FFI ni
arrancar la JVM. Es la capa de testing más rápida y más cercana a la fuente del defecto.

### Herramienta
`cargo test --lib` — Cargo compila un binario de test adicional (rlib) sobre el mismo
código fuente. Compatible con `crate-type = ["cdylib", "staticlib"]` desde Rust 1.70.

### Módulos a cubrir

| Archivo | Función | Tests clave |
|---------|---------|------------|
| `error.rs` | thread-local LAST_ERROR | set/clear/copy; string con nuls internos; aislamiento entre hilos |
| `util.rs` | `cstr_to_str` | null ptr → None; UTF-8 válido → Some; bytes inválidos → None |
| `handle.rs` | `handle_ref`, RawConn::Drop | null → None; puntero válido → Some; Drop llama sqlite3_close |
| `stmt.rs` | `stmt_ref` | null → None; puntero válido → Some |
| `connection.rs` | `snr_open*`, `snr_close`, `snr_exec`, ... | ruta válida, inválida; memory con nombre inválido; flag ALLOWED_FLAGS |
| `statement.rs` | `snr_prepare`, `snr_bind_*`, `snr_column_*`, `snr_step` | todos los tipos de bind/column; índices fuera de rango; step ROW/DONE/ERROR |
| `transaction.rs` | BEGIN/COMMIT/ROLLBACK/SAVEPOINT | flujo normal; doble BEGIN; RELEASE savepoint inexistente |
| `wal.rs` | `snr_wal_checkpoint`, `snr_wal_autocheckpoint` | PASSIVE/FULL/RESTART/TRUNCATE; nombre BD inválido |

### Cobertura objetivo
≥ 90% líneas medido con `cargo-llvm-cov --lib`.

### Patrón de test en Rust

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn open_memory() -> *mut Handle {
        unsafe { snr_open_memory(std::ptr::null()) }
    }

    #[test]
    fn exec_ok() {
        let h = open_memory();
        assert!(!h.is_null());
        let sql = CString::new("CREATE TABLE t (x INTEGER)").unwrap();
        let rc = unsafe { snr_exec(h, sql.as_ptr()) };
        assert_eq!(rc, 0);
        unsafe { snr_close(h) };
    }

    #[test]
    fn exec_null_handle_returns_error() {
        let sql = CString::new("SELECT 1").unwrap();
        let rc = unsafe { snr_exec(std::ptr::null_mut(), sql.as_ptr()) };
        assert_eq!(rc, -1);
        // last_error debe estar poblado
        let err_ptr = snr_last_error();
        assert!(!err_ptr.is_null());
    }
}
```

### Coste estimado
~100 tests, ~4-6 horas de implementación. `cargo test --lib` < 5 segundos.

---

## 4. TT-2 — FFI Contract Tests

### Objetivo
Verificar el **contrato C ABI** desde el punto de vista de un consumidor externo.
A diferencia de TT-1, estos tests compilan en una unidad separada (`tests/`) y usan
los símbolos exportados como lo haría un linker externo.

### Herramienta
`cargo test --test` — tests en `sources/rust/ether-sqlite/tests/`.
Validan que `#[no_mangle]` funciona, que las firmas son correctas y que el contrato
de ownership de memoria (caller-frees vs internal ptr) se cumple.

### Contratos a verificar

| Contrato | Descripción | Test |
|---------|-------------|------|
| Null safety | Todo `snr_*` con handle NULL retorna -1 / NULL sin crash | `null_handle_*` |
| Ownership transfer | `snr_last_error_copy` → `snr_free_string` libera sin crash | `free_after_copy` |
| No double-free | `snr_free_string(null)` es no-op | `free_null_is_noop` |
| Error isolation | Thread A error no visible en Thread B | `thread_error_isolation` |
| Mutex concurrencia | Dos hilos en mismo handle → serialización correcta | `concurrent_exec` |
| `snr_close` con stmts | Close de conexión con statement abierto → stmt sigue válido | `close_with_open_stmt` |
| `blob len < 0` rechazado | `snr_bind_blob(stmt, 1, ptr, -1)` retorna -1 | `blob_negative_len` |
| `memory` name allowlist | Caracteres fuera de `[A-Za-z0-9_-]` → NULL + error | `memory_invalid_name_*` |

### Distinción crítica respecto a TT-1
TT-1 puede llamar funciones `pub(crate)`. TT-2 sólo puede acceder a símbolos
`pub` exportados por el crate — exactamente lo que Java verá vía Panama FFM.

### Coste estimado
~35 tests, ~2-3 horas. `cargo test --test` < 10 segundos.

---

## 5. TT-3 — Java Integration Tests

### Objetivo
Verificar escenarios **realistas de uso** que no caben en unit tests: concurrencia,
múltiples conexiones, grandes datasets, recovery de errores, WAL bajo carga.

### Diferencia con Java unit tests existentes
Los unit tests verifican la API contrato (cada método en aislamiento). Los
integration tests verifican **comportamiento emergente** del stack completo.

### Escenarios prioritarios

| Escenario | Descripción | Riesgo cubierto |
|-----------|-------------|-----------------|
| `ConcurrentReadsTest` | N virtual threads leyendo la misma BD | Mutex + FULLMUTEX + Loom |
| `ConcurrentWritesTest` | N virtual threads escribiendo + SQLITE_BUSY handling | Retry con busyTimeout |
| `MultiConnectionWalTest` | Escritor + lector en WAL mode con file real | WAL + checkpoint |
| `BulkInsertTest` | 10k+ INSERT en transacción + batch SELECT | Streaming, memory |
| `LargeTextBlobTest` | TEXT y BLOB de 1 MB+ | `columnTextSafe` vs `columnText` |
| `StatementReuseTest` | 1000 ciclos reset/rebind/step del mismo statement | Resource lifecycle |
| `ConnectionPoolSimTest` | 50 conexiones open/close en loop | Arc lifecycle, no leaks |
| `ErrorRecoveryTest` | Excepción en transaction → rollback → reuso de conexión | Error path + recovery |
| `SavepointNestingTest` | Savepoints anidados, rollback parcial | Semántica transaccional |
| `WalCheckpointLoadTest` | WAL grow + checkpoint TRUNCATE | Observabilidad WAL |

### Maven setup

```xml
<!-- perfil integration en pom.xml -->
<profile>
  <id>integration</id>
  <build>
    <plugins>
      <plugin>
        <groupId>org.apache.maven.plugins</groupId>
        <artifactId>maven-surefire-plugin</artifactId>
        <configuration>
          <groups>integration</groups>
          <argLine>@{argLine} --enable-native-access=ALL-UNNAMED</argLine>
        </configuration>
      </plugin>
    </plugins>
  </build>
</profile>
```

```java
@Tag("integration")
class ConcurrentReadsTest { ... }
```

Por defecto, surefire excluye el tag `integration`. Se activan con `mvn test -Pintegration`.

### Makefile target

```makefile
test-integration: build-rust
    cd $(JAVA_DIR) && \
      SNR_LIB=$(SNR_LIB) \
      JAVA_HOME=$(GRAALVM_HOME) \
      $(MVNW) test -Pintegration
```

### Coste estimado
~40 tests, ~4-6 horas. Algunos tests pueden tardar segundos (WAL, bulk insert).

---

## 6. TT-4 — GraalVM Native Image Tests

### Objetivo
Verificar que el proyecto funciona correctamente compilado como **GraalVM Native Image**:
- `SqliteLibrary` se inicializa en runtime (no build time)
- Los downcalls Panama FFM funcionan en AOT
- `SymbolLookup.libraryLookup` carga la `.so`/`.dylib` correctamente en binario nativo
- No hay clases/métodos accedidos via reflection no registrados

### Fases

#### Fase 1 — SmokeTest automatizado (TT-4a)
Extender y automatizar el `SmokeTest` existente:
- Compilar con `mvn -Pnative package native:compile`
- Ejecutar `./target/snr-smoke` y verificar exit code 0 + output esperado
- Añadir `make native-test` que falla si el binario no produce la salida correcta
- Integrar en CI (DT-5)

#### Fase 2 — JUnit 5 nativo (TT-4b, futuro)
Usar `native-maven-plugin` goal `native:test`:
- Compila los tests JUnit 5 como binario nativo
- Los ejecuta y reporta resultados
- Requiere GraalVM Build Tools con soporte de Panama FFM en native test

```xml
<!-- Requerirá en pom.xml perfil native-test -->
<plugin>
  <groupId>org.graalvm.buildtools</groupId>
  <artifactId>native-maven-plugin</artifactId>
  <configuration>
    <buildArgs>
      <buildArg>--initialize-at-run-time=mx.rafex.sqlite.SqliteLibrary</buildArg>
      <buildArg>--enable-native-access=ALL-UNNAMED</buildArg>
    </buildArgs>
  </configuration>
  <executions>
    <execution>
      <id>native-test</id>
      <goals><goal>test</goal></goals>
    </execution>
  </executions>
</plugin>
```

### Problemas conocidos en Native Image

| Componente | Estado | Notas |
|-----------|--------|-------|
| `Arena.ofConfined()` | ✅ Funciona | GraalVM 22+ |
| `SymbolLookup.libraryLookup(path, Arena.global())` | ✅ Funciona | biblioteca debe existir en runtime |
| `Linker.nativeLinker().downcallHandle(...)` | ✅ Funciona | no requiere reflection |
| `FunctionDescriptor` | ✅ Funciona | tipos primitivos directos |
| `MethodHandle.invokeExact()` | ✅ Funciona | AOT-compilable |
| JUnit 5 platform en native | ⚠️ Requiere config | GraalVM Build Tools 0.10+ |
| `--initialize-at-run-time=SqliteLibrary` | ✅ Ya configurado | en perfil `native` |

### Coste estimado
- TT-4a: ~2 horas (ampliar SmokeTest + Makefile target)
- TT-4b: ~4-6 horas (JUnit native, requiere CI con GraalVM)

---

## 7. Decisiones de diseño de tests

### DEC-TEST-1 — cargo test con cdylib+staticlib
`cargo test --lib` añade internamente `rlib` al build para el test binary.
Compatible con la configuración actual sin cambios en Cargo.toml. Decisión: mantener
`crate-type = ["cdylib", "staticlib"]` y usar `cargo test --lib` + `cargo test --test`.

### DEC-TEST-2 — Rust coverage tool
`cargo-llvm-cov` (stable, llvm-tools) vs `cargo-tarpaulin` (instrumentación de proceso).
Decisión: `cargo-llvm-cov` — más preciso en aarch64-apple-darwin, genera LCOV compatible con CI.
Instalación: `cargo install cargo-llvm-cov --locked`.

### DEC-TEST-3 — Java integration tests: mismo módulo vs módulo separado
Módulo separado aísla dependencias y tiempos de build. Mismo módulo es más simple.
Decisión: mismo módulo + `@Tag("integration")` + perfil Maven. Extraer si crece.

### DEC-TEST-4 — GraalVM native: SmokeTest vs JUnit nativo
JUnit nativo (TT-4b) tiene alta complejidad y poca documentación con Panama FFM.
SmokeTest (TT-4a) está ya parcialmente implementado y es inmediatamente útil.
Decisión: TT-4a primero, TT-4b como deuda técnica DT-8.

---

## 8. Orden de implementación y dependencias

```
TT-1 (Rust unit)
  └── sin dependencias — primero

TT-2 (FFI contract)
  └── depende de: TT-1 completado (reutiliza helpers)

TT-3 (Java integration)
  └── depende de: build .dylib (ya existe), perfil Maven nuevo

TT-4a (Native SmokeTest)
  └── depende de: GraalVM instalado, make native (ya funciona)
  └── bloquea: DT-5 CI pipeline

TT-4b (JUnit nativo, futuro)
  └── depende de: TT-4a + DT-5 CI + investigación GraalVM Build Tools
```

---

## 9. Integración en Makefile

```makefile
# TT-1
test-rust:
    cd $(RUST_DIR) && RUSTC=$(RUSTC) $(CARGO) test --lib

# TT-1 + TT-2
test-rust-all:
    cd $(RUST_DIR) && RUSTC=$(RUSTC) $(CARGO) test

# TT-1 + TT-2 + cobertura Rust
coverage-rust:
    cd $(RUST_DIR) && RUSTC=$(RUSTC) $(CARGO) llvm-cov --lib --lcov --output-path target/lcov.info

# TT-3
test-integration: build-rust
    cd $(JAVA_DIR) && SNR_LIB=$(SNR_LIB) JAVA_HOME=$(GRAALVM_HOME) \
      $(MVNW) test -Pintegration

# TT-4a
native-test: build
    cd $(JAVA_DIR) && SNR_LIB=$(SNR_LIB) JAVA_HOME=$(GRAALVM_HOME) \
      $(MVNW) -Pnative package native:compile -q
    SNR_LIB=$(SNR_LIB) $(JAVA_DIR)/target/snr-smoke

# todos los tests
test-all: test-rust-all test-unit test-integration
```

---

## 10. Resumen ejecutivo

| Iniciativa | Tests | Tiempo build | Esfuerzo impl. | Prioridad |
|-----------|-------|-------------|----------------|-----------|
| TT-1 Rust unit | ~100 | < 5 s | 4-6 h | Alta |
| TT-2 FFI contract | ~35 | < 10 s | 2-3 h | Alta |
| TT-3 Java integration | ~40 | 10-30 s | 4-6 h | Media |
| TT-4a Native SmokeTest | ~15 escenarios | 2-4 min | 2 h | Media |
| TT-4b JUnit nativo | todos | 5-10 min | 4-6 h | Baja |

**Resultado final esperado (TT-1 + TT-2 + TT-3 + TT-4a):**
- Rust: ≥ 90% cobertura líneas
- Java unit: 99% LINE (existente)
- Java integration: escenarios reales validados
- Native: SmokeTest automatizado en CI
