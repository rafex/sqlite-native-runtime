# PLAN — sqlite-native-runtime · Revisión de seguridad y hardening

**Última actualización:** 2026-05-21
**Estado general:** Correcciones de seguridad completas. Revisión arquitectónica completada (R-1..R-9).
**Commit base:** `7b25047` · **Commit hotfixes CRÍTICO/ALTO:** `c7989d6` · **Commit MEDIO/INFO:** `411a66f` · **Commit deuda técnica:** `730effd` · **Commit revisión arquitectónica:** `TBD`

---

## Contexto

Se realizó una revisión completa de seguridad sobre el codebase Rust+Java de
`sqlite-native-runtime` (librería SQLite genérica para Java 21 + GraalVM Native Image via Panama FFI).

La revisión identificó **11 hallazgos** clasificados en cuatro niveles de severidad:

| Severidad | Total | Corregidos | Pendientes |
|-----------|-------|------------|------------|
| CRÍTICO   | 3     | 3 ✅       | 0          |
| ALTO      | 4     | 4 ✅       | 0          |
| MEDIO     | 3     | 3 ✅       | 0          |
| INFORMATIVO | 2   | 2 ✅       | 0          |
| **Total** | **12**| **12 ✅**  | **0**      |

---

## Progreso General

| Iniciativa | Estado | % | Commit | Horas |
|------------|--------|---|--------|-------|
| **SEC-CRITICO · Hotfixes C-1, C-2, C-3** | ✅ Completada | 100% | `c7989d6` | 2 h |
| **SEC-ALTO · Hotfixes A-1, A-2, A-3, A-4** | ✅ Completada | 100% | `c7989d6` | 2 h |
| **SEC-INFORMATIVO · Hotfix I-2** | ✅ Completada | 100% | `c7989d6` | 0.5 h |
| **SEC-MEDIO · M-1, M-2, M-3** | ✅ Completada | 100% | `411a66f` | 2 h |
| **SEC-INFO · I-1 ruta relativa Java** | ✅ Completada | 100% | `411a66f` | 0.5 h |
| **ARCH · Revisión arquitectónica R-1..R-9** | ✅ Completada | 100% | `TBD` | 3 h |
| **BUILD · Cross-compilación Linux** | ⏳ Pendiente | 0% | — | ~2 h |
| **CI · Smoke test automatizado** | ⏳ Pendiente | 0% | — | ~2 h |

---

## ✅ HOTFIXES APLICADOS

Todos los hallazgos CRÍTICO y ALTO se corrigieron en un único commit `c7989d6`
con mensaje completo. El smoke test (`mx.rafex.sqlite.SmokeTest`) pasa en su
totalidad tras los cambios.

---

### C-1 · Use-after-free en `snr_close` — CRÍTICO ✅

**Archivos:** `rust/src/handle.rs`, `rust/src/connection.rs`

**Problema:** `snr_close` accedía a campos del `Handle` después de liberar la
memoria con `Box::from_raw`. Si además había `StmtHandle` activos, `sqlite3_close`
se invocaba dos veces (una manual, otra en `Drop`), produciendo double-free.

**Fix aplicado:**

- `RawConn` implementa `Drop` que llama `sqlite3_close` cuando el `Arc` llega
  a `refcount=0`. Si hay statements abiertos, el Arc permanece vivo en ellos y
  el cierre se difiere automáticamente.
- `snr_close` simplificado a `drop(Box::from_raw(handle))` — cero lógica manual.

```rust
// handle.rs — Drop garantiza cierre exactamente una vez
impl Drop for RawConn {
    fn drop(&mut self) {
        if !self.0.is_null() {
            let rc = unsafe { libsqlite3_sys::sqlite3_close(self.0) };
            // ...
        }
    }
}

// connection.rs — snr_close ahora es trivial
pub unsafe extern "C" fn snr_close(handle: *mut Handle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}
```

**Evidencia de cierre:** `cargo check` + `cargo build --release` sin errores. Smoke test pasa.

---

### C-2 · SQL injection en nombres de savepoint — CRÍTICO ✅

**Archivo:** `rust/src/transaction.rs`

**Problema:** `exec_with_name` construía `SAVEPOINT "nombre"` sin escapar
comillas dobles. Un nombre como `x" ; DROP TABLE items; --` se ejecutaba
como SQL arbitrario.

**Fix aplicado:**

```rust
// transaction.rs:140
let safe_name = name_str.replace('"', "\"\"");
let sql = format!("{command} \"{safe_name}\"\0");
```

Escape estándar SQL de identificadores: `"` → `""`.
Aplica a `snr_savepoint`, `snr_release` y `snr_rollback_to`.

**Evidencia de cierre:** Smoke test incluye `withSavepoint("sp1", ...)` que pasa sin errores.

---

### C-3 · UB en `SQLITE_TRANSIENT` via transmute — CRÍTICO ✅

**Archivo:** `rust/src/statement.rs`

**Problema:** El código usaba:
```rust
Some(std::mem::transmute::<isize, unsafe extern "C" fn(*mut c_void)>(-1isize))
```
Transmuting un entero a un puntero de función es **Undefined Behavior** en Rust.
El compilador lo detecta como UB en contextos `const`. Afectaba a `snr_bind_text`
y `snr_bind_blob`.

**Fix aplicado:**

`libsqlite3-sys 0.30` exporta `SQLITE_TRANSIENT()` como función:
```rust
// statement.rs — usando la función oficial del crate
let rc = ffi::sqlite3_bind_text(sh.stmt, idx, val, -1, ffi::SQLITE_TRANSIENT());
let rc = ffi::sqlite3_bind_blob(sh.stmt, idx, data as *const _, len, ffi::SQLITE_TRANSIENT());
```

**Evidencia de cierre:** `cargo check` limpio. Smoke test valida bind de texto y blob.

---

### A-1 · Directory traversal via `create_dir_all` — ALTO ✅

**Archivo:** `rust/src/connection.rs`

**Problema:** `snr_open` llamaba `std::fs::create_dir_all(parent)` sobre la
ruta que Java pasaba como argumento. Un path como `../../etc/cron.d/evil`
creaba directorios arbitrarios en el sistema de archivos.

**Fix aplicado:** Bloque eliminado íntegramente. El directorio padre debe existir
antes de llamar `snr_open`. Se añadió comentario explicativo:

```rust
// El directorio padre debe existir antes de llamar snr_open.
// Crear directorios arbitrarios desde una librería FFI es un vector de
// directory-traversal — se elimina deliberadamente.
```

**Evidencia de cierre:** El código ya no contiene `create_dir_all`.

---

### A-2 · URI injection en `snr_open_memory` — ALTO ✅

**Archivo:** `rust/src/connection.rs`

**Problema:** El nombre de la base de datos en memoria se interpolaba directamente
en `file:{name}?mode=memory&cache=shared`. Un nombre como `x?mode=ro&cache=shared`
permitía cambiar los flags de apertura; un nombre con `/` o `..` podría abrir un
archivo real del disco.

**Fix aplicado:**

```rust
if !n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
    set_last_error("snr_open_memory: name inválido — solo [A-Za-z0-9_-] permitido");
    return std::ptr::null_mut();
}
```

Allowlist estricta: solo caracteres seguros para identificadores.

**Evidencia de cierre:** Smoke test usa `snr_open_memory(null)` (`:memory:`). La validación se aplica solo cuando `name != null`.

---

### A-3 · Buffer over-read en `snr_bind_blob` con `len < 0` — ALTO ✅

**Archivo:** `rust/src/statement.rs`

**Problema:** `sqlite3_bind_blob` interpreta `len < 0` como "dato nul-terminado"
y lee hasta encontrar un byte `\0`. Si Java pasa un `byte[]` sin terminador nulo,
SQLite lee más allá del buffer — buffer over-read.

**Fix aplicado:**

```rust
if len < 0 {
    set_last_error("snr_bind_blob: len negativo no permitido");
    return -1;
}
```

**Evidencia de cierre:** Smoke test valida `snr_bind_blob` con array de 4 bytes.

---

### A-4 · Mutex poisoning ignorado silenciosamente — ALTO ✅

**Archivo:** `rust/src/statement.rs`

**Problema:** Todas las funciones de statement usaban `let _guard = sh.conn.lock();`
ignorando el `Result`. Si el mutex estaba envenenado (otro hilo entró en panic
mientras lo sostenía), el código continuaba sin el lock — race condition silenciosa.

**Fix aplicado en ~18 puntos:**

```rust
// Patrón para funciones que retornan i32
let _guard = match sh.conn.lock() {
    Ok(g) => g,
    Err(_) => { set_last_error("fn: mutex envenenado"); return -1; }
};

// snr_stmt_close — siempre finaliza aunque haya poison
let _guard = sh.conn.lock().unwrap_or_else(|e| e.into_inner());
```

Funciones afectadas: `snr_stmt_close`, `snr_stmt_reset`, `snr_stmt_clear_bindings`,
`snr_bind_null/int/double/text/blob`, `snr_bind_parameter_index`, `snr_step`,
`snr_column_count/type/int/double/text/text_owned/blob/bytes/name`.

**Evidencia de cierre:** `cargo check` limpio sin warnings relacionados.

---

### I-2 · Flags de apertura sin filtrar — INFORMATIVO ✅

**Archivo:** `rust/src/connection.rs`

**Problema:** Java podía pasar flags internos de SQLite (`SQLITE_OPEN_DELETEONCLOSE`,
`SQLITE_OPEN_TEMP_DB`, etc.) que abrirían comportamientos no deseados.

**Fix aplicado:**

```rust
const ALLOWED_FLAGS: i32 = ffi::SQLITE_OPEN_READONLY as i32
    | ffi::SQLITE_OPEN_READWRITE as i32
    | ffi::SQLITE_OPEN_CREATE as i32
    | ffi::SQLITE_OPEN_URI as i32
    | ffi::SQLITE_OPEN_NOFOLLOW as i32; // rechaza symlinks

let open_flags = if flags == 0 {
    ffi::SQLITE_OPEN_READWRITE | ffi::SQLITE_OPEN_CREATE | ffi::SQLITE_OPEN_FULLMUTEX
} else {
    (flags & ALLOWED_FLAGS) | ffi::SQLITE_OPEN_FULLMUTEX
};
```

`SQLITE_OPEN_FULLMUTEX` siempre forzado. `SQLITE_OPEN_NOFOLLOW` en el allowlist
para que el caller pueda rechazar symlinks.

---

## ✅ TAREAS COMPLETADAS (MEDIO / INFORMATIVO)

---

### TASK-0001 — M-1 · Thread-local `snr_last_error` con Project Loom ✅

- **ID:** TASK-0001  **Severidad:** MEDIO  **Estado:** `done`  **Commit:** `411a66f`
- **Archivos:** `rust/src/error.rs`, `SqliteLibrary.java`, `SqliteStatement.java`

**Solución aplicada:**

Añadida `snr_last_error_copy()` en Rust que devuelve una copia heap del error:
```rust
pub extern "C" fn snr_last_error_copy() -> *mut c_char {
    LAST_ERROR.with(|cell| {
        match cell.borrow().as_ref() {
            None => std::ptr::null_mut(),
            Some(cs) => match CString::new(cs.as_bytes()) {
                Ok(copy) => copy.into_raw(),
                Err(_) => std::ptr::null_mut(),
            }
        }
    })
}
```

`SqliteStatement.lastError()` cambiado a `readAndFreeString(SqliteLibrary.snr_last_error_copy())`.
Javadoc de `snr_last_error()` ampliado con advertencia explícita de virtual threads.
`snr_last_error()` original se mantiene para código single-threaded (sin alloc).

---

### TASK-0002 — M-2 · `readInternalString` y bound de `columnText` ✅

- **ID:** TASK-0002  **Severidad:** MEDIO  **Estado:** `done`  **Commit:** `411a66f`
- **Archivo:** `java/src/main/java/mx/rafex/sqlite/SqliteStatement.java`

**Solución aplicada:**

`readInternalString` usa `getString(0, UTF_8)` (API oficial Panama) vía
`ptr.reinterpret(Long.MAX_VALUE)` — evita el loop manual byte-a-byte.

`columnText(int col)` acota el segmento al tamaño exacto reportado por `snr_column_bytes`:
```java
public String columnText(int col) {
    var ptr = SqliteLibrary.snr_column_text(stmt, col);
    if (ptr == null || MemorySegment.NULL.equals(ptr)) return null;
    int byteLen = SqliteLibrary.snr_column_bytes(stmt, col);
    if (byteLen <= 0) return "";
    return ptr.reinterpret(byteLen + 1L).getString(0, StandardCharsets.UTF_8);
}
```

Panama FFI lanzará `IndexOutOfBoundsException` si SQLite devuelve un texto más
largo que `byteLen + 1` bytes — detección temprana de corrupción.

---

### TASK-0003 — M-3 · Sin límite de longitud en SQL — decisión documentada ✅

- **ID:** TASK-0003  **Severidad:** MEDIO  **Estado:** `done`  **Commit:** `411a66f`
- **Decisión:** Ver DEC-0005 más abajo.

`snr_exec` y `snr_prepare` no aplican límite de longitud al SQL. Esta es una
decisión consciente: la librería es de uso interno, no expuesta directamente a
input no-confiable de red. Si el caller expone `snr_exec` vía API HTTP u otro
boundary externo, el caller es responsable de validar la longitud.

---

### TASK-0004 — I-1 · Rutas relativas en `loadLibrary()` ✅

- **ID:** TASK-0004  **Severidad:** INFORMATIVO  **Estado:** `done`  **Commit:** `411a66f`
- **Archivo:** `java/src/main/java/mx/rafex/sqlite/SqliteLibrary.java`

**Solución aplicada:**

Las rutas CWD se mantienen como fallback de desarrollo pero emiten un warning
en stderr si se cargan:
```java
if (!candidate.isAbsolute()) {
    System.err.println("[snr] AVISO: cargando desde CWD: " + abs
        + " — en producción define snr.lib o SNR_LIB con ruta absoluta.");
}
```
Comentario de "solo para desarrollo" añadido al bloque de candidatos relativos.

---

---

## ✅ REVISIÓN ARQUITECTÓNICA (R-1..R-9)

Análisis como arquitecto de software ejecutado en `2026-05-21`. Nueve hallazgos corregidos.

---

### R-1 · `SqliteConnection.lastError()` usaba puntero interno inseguro con Loom ✅

**Archivo:** `java/.../SqliteConnection.java`

`lastError()` usaba `readInternalString(snr_last_error())` — el mismo puntero thread-local
que se advirtió en M-1. Con virtual threads, el mensaje en la excepción podía corresponder
a un error de otro virtual thread.

**Fix:** cambiado a `readAndFreeString(snr_last_error_copy())`, consistente con
`SqliteStatement.lastError()`.

---

### R-2 · Modelo de threading no documentado en `SqliteStatement` ✅

**Archivo:** `java/.../SqliteStatement.java`

Añadida sección "Modelo de threading" al Javadoc de clase explicando que el objeto
no es thread-safe para secuencias de llamadas (cada llamada individual está serializada,
pero secuencias como `columnText()` + `columnBytes()` no son atómicas).

---

### R-3 · `SQLITE_OPEN_NOFOLLOW` ausente de los flags por defecto ✅

**Archivo:** `rust/src/connection.rs`

El default `flags == 0` no incluía `SQLITE_OPEN_NOFOLLOW`. Un atacante con acceso
al filesystem podría plantar un symlink para redirigir la apertura.

**Fix:**
```rust
ffi::SQLITE_OPEN_READWRITE | ffi::SQLITE_OPEN_CREATE
    | ffi::SQLITE_OPEN_FULLMUTEX | ffi::SQLITE_OPEN_NOFOLLOW
```

---

### R-4 · WAL checkpoint descartaba n_log y n_ckpt ✅

**Archivos:** `rust/src/wal.rs`, `SqliteLibrary.java`, `SqliteConnection.java`

`sqlite3_wal_checkpoint_v2` calculaba los frames WAL pero los descartaba. Sin esta
información es imposible diagnosticar WAL growth en producción.

**Fix:** `snr_wal_checkpoint` recibe dos punteros de salida opcionales `*mut i32`:

```rust
pub unsafe extern "C" fn snr_wal_checkpoint(
    handle, db_name, mode,
    out_wal_frames: *mut i32,   // nullable
    out_checkpointed: *mut i32, // nullable
) -> i32
```

`SqliteConnection.walCheckpoint()` ahora devuelve `WalCheckpointResult(walFrames, checkpointed)`:

```java
WalCheckpointResult r = db.walCheckpoint(WalMode.TRUNCATE, null);
// r.walFrames(), r.checkpointed()
```

---

### R-5 · Doble capa de locking documentada ✅

**Archivo:** `rust/src/handle.rs`

Con `SQLITE_OPEN_FULLMUTEX`, SQLite serializa internamente. El `Mutex<RawConn>` en Rust
no añade serialización de operaciones — su rol es proporcionar `Send` seguro al Arc y
garantizar exclusión mutua en el `Drop`. Documentado con comentario extendido en `Handle`.

---

### R-6 · Constantes de apertura exportadas como funciones ✅

**Archivo:** `java/.../SqliteLibrary.java`

`snr_flag_readonly()`, `snr_flag_readwrite()`, `snr_flag_create()` eran downcalls FFI
para devolver constantes conocidas en tiempo de compilación. Añadidas como constantes Java:

```java
public static final int OPEN_READONLY  = 0x00000001;
public static final int OPEN_READWRITE = 0x00000002;
public static final int OPEN_CREATE    = 0x00000004;
public static final int OPEN_NOFOLLOW  = 0x01000000;
```

Los valores son parte de la especificación SQLite y no cambiarán.

---

### R-7 · `clear_last_error()` inconsistente en funciones bind y column ✅

**Archivo:** `rust/src/statement.rs`

Las funciones `snr_bind_null/int/double/text/blob`, `snr_bind_parameter_index` y todas
las `snr_column_*` no llamaban `clear_last_error()` al inicio. Un error previo
podía persistir aunque la operación siguiente tuviera éxito.

**Fix:** `clear_last_error()` añadida al inicio de todas las funciones afectadas.
Además, `snr_bind_parameter_index` ahora establece error explícito si `name` es NULL
(antes devolvía 0 silenciosamente sin distinguir "parámetro no encontrado" de "name nulo").

---

### R-8 · `snr_column_text_owned` hacía dos allocaciones ✅

**Archivo:** `rust/src/statement.rs`

```rust
// Antes: String + CString (dos heap allocations)
let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
CString::new(s)?.into_raw()

// Después: CString directo (una sola allocation)
CStr::from_ptr(ptr as *const c_char).to_owned().into_raw()
```

---

### R-9 · Sin detección de resource leaks al cerrar conexión ✅

**Archivos:** `SqliteConnection.java`, `SqliteStatement.java`

`SqliteStatement` acepta un `Runnable onClose` opcional. `SqliteConnection.prepare()`
lo usa para mantener un `AtomicInteger openStatements`. Al llamar `close()` en la conexión
se emite un `WARNING` en el logger si hay statements aún abiertos:

```
[snr] conexión cerrada con N statement(s) aún abiertos — posible resource leak.
```

---

### R-panic · `set_last_error` podía hacer panic a través del boundary FFI ✅

**Archivo:** `rust/src/error.rs`

El fallback usaba `.expect("static string")`. Un panic a través del boundary FFI es UB en Rust.

**Fix:** truncar en el primer byte nulo con `from_vec_unchecked` — nunca puede fallar:
```rust
let nul_pos = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
let cs = unsafe { CString::from_vec_unchecked(bytes[..nul_pos].to_vec()) };
```

También, `snr_last_error_copy()` reemplaza el `CString::new(cs.as_bytes()).unwrap_or_else`
por `CStr::to_owned()` que clona directamente sin poder fallar.

---

## ⏳ PENDIENTE

---

## Deuda técnica menor

| ID | Archivo | Descripción | Prioridad | Estado |
|----|---------|-------------|-----------|--------|
| DT-1 | `rust/src/util.rs` | `cstr_to_string()` sin usar — eliminada | Baja | ✅ |
| DT-2 | `rust/src/` | `cargo check` limpio — 0 warnings tras eliminar DT-1 | Media | ✅ |
| DT-3 | `Makefile` | No hay target `cross` para compilar Linux aarch64/amd64 | Media | ⏳ |
| DT-4 | `java/` | `mvn package` genera JAR pero no hay `mvn install` automático | Baja | ⏳ |
| DT-5 | CI | No hay pipeline GitHub Actions para ejecutar smoke test en PR | Alta | ⏳ |
| DT-6 | `README.md` | Actualizado a Java 22 + JEP 454 + `snr_last_error_copy` en tabla ABI | Baja | ✅ |

---

## Decisiones de diseño registradas

### DEC-0001 — `libsqlite3-sys` directo, no `rusqlite`

- **Fecha:** 2026-05-21
- **Estado:** `accepted`
- **Contexto:** La librería es genérica para ser consumida desde Java via Panama FFI. `rusqlite` añade abstracciones Rust-idiomáticas que no aportan valor en un binding C ABI.
- **Decisión:** Usar `libsqlite3-sys` con `features = ["bundled"]` directamente.
- **Consecuencias:** Mayor control sobre la ABI; sin SQLite en el sistema operativo como dependencia runtime.

### DEC-0002 — Panama FFI, no JNI

- **Fecha:** 2026-05-21
- **Estado:** `accepted`
- **Contexto:** JNI requiere reflection y extracción de `.so` a `/tmp`, incompatible con GraalVM Native Image sin configuración extensiva.
- **Decisión:** Panama FFI estable (Java 22, JEP 454). Flag `--enable-native-access=ALL-UNNAMED` en runtime.
- **Consecuencias:** GraalVM solo necesita `--initialize-at-run-time=mx.rafex.sqlite.SqliteLibrary`.

### DEC-0003 — `sqlite3_close` no `sqlite3_close_v2`

- **Fecha:** 2026-05-21
- **Estado:** `accepted`
- **Contexto:** `libsqlite3-sys 0.30` no expone `sqlite3_close_v2`. El diseño con `Arc<Mutex<RawConn>>` garantiza que todos los statements se finalizan antes de que el `Arc` llegue a `refcount=0`, por lo que `sqlite3_close` nunca retorna `SQLITE_BUSY` en uso correcto.
- **Decisión:** Usar `sqlite3_close` en `RawConn::Drop`.
- **Consecuencias:** Si el caller ignora el error de `snr_stmt_close`, hay riesgo de `SQLITE_BUSY` en `Drop`. El eprintln actúa como señal de error de programador.

### DEC-0005 — Sin límite de longitud SQL en `snr_exec`/`snr_prepare`

- **Fecha:** 2026-05-21
- **Estado:** `accepted`
- **Contexto:** M-3 planteó añadir un guard de longitud máxima en SQL. La librería es de uso interno (dependency Maven de proyectos Java propios), no un servicio de red expuesto directamente.
- **Decisión:** No aplicar límite de longitud. El caller valida si expone el ABI a input externo. Documentado como invariant en Javadoc de `snr_exec`.
- **Consecuencias:** Si un proyecto usa `snr_exec` para ejecutar SQL construido desde input de usuario sin sanitizar, el SQL puede ser arbitrariamente largo. Esto es un error de diseño del caller, no de la librería.

### DEC-0004 — Thread-local para `snr_last_error`

- **Fecha:** 2026-05-21
- **Estado:** `accepted`
- **Contexto:** La alternativa (global con mutex) añadiría contención en hot paths. La librería está pensada para uso en un único hilo OS por conexión.
- **Decisión:** `thread_local! { static LAST_ERROR: RefCell<Option<CString>> }`.
- **Consecuencias:** Ver TASK-0001 (M-1) — riesgo con Project Loom / virtual threads. Documentar la limitación.

---

## Trazabilidad

| Hallazgo | Severidad | Archivo(s) | Commit | Estado |
|----------|-----------|------------|--------|--------|
| C-1 use-after-free | CRÍTICO | handle.rs, connection.rs | `c7989d6` | ✅ Corregido |
| C-2 SQL injection savepoint | CRÍTICO | transaction.rs | `c7989d6` | ✅ Corregido |
| C-3 SQLITE_TRANSIENT UB | CRÍTICO | statement.rs | `c7989d6` | ✅ Corregido |
| A-1 directory traversal | ALTO | connection.rs | `c7989d6` | ✅ Corregido |
| A-2 URI injection | ALTO | connection.rs | `c7989d6` | ✅ Corregido |
| A-3 buffer over-read blob | ALTO | statement.rs | `c7989d6` | ✅ Corregido |
| A-4 mutex poisoning | ALTO | statement.rs | `c7989d6` | ✅ Corregido |
| M-1 thread-local + Loom | MEDIO | error.rs, SqliteLibrary.java | `411a66f` | ✅ TASK-0001 |
| M-2 reinterpret MAX_VALUE | MEDIO | SqliteStatement.java | `411a66f` | ✅ TASK-0002 |
| M-3 sin límite SQL length | MEDIO | PLAN.md DEC-0005 | `411a66f` | ✅ TASK-0003 |
| I-1 rutas relativas dylib | INFO | SqliteLibrary.java | `411a66f` | ✅ TASK-0004 |
| I-2 flags sin filtrar | INFO | connection.rs | `c7989d6` | ✅ Corregido |

---

## Referencias

- [rust/src/connection.rs](rust/src/connection.rs) — ABI de conexión
- [rust/src/statement.rs](rust/src/statement.rs) — ABI de statements y bind/column
- [rust/src/transaction.rs](rust/src/transaction.rs) — ABI de transacciones y savepoints
- [rust/src/handle.rs](rust/src/handle.rs) — RawConn + Arc lifecycle
- [rust/src/error.rs](rust/src/error.rs) — thread-local LAST_ERROR
- [java/src/main/java/mx/rafex/sqlite/SqliteLibrary.java](java/src/main/java/mx/rafex/sqlite/SqliteLibrary.java) — bindings Panama FFI
- [java/src/main/java/mx/rafex/sqlite/SqliteStatement.java](java/src/main/java/mx/rafex/sqlite/SqliteStatement.java) — wrapper alto nivel
- [README.md](README.md) — ABI exportada completa (48 símbolos `snr_*`)
