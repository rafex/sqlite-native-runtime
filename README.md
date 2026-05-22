# sqlite-native-runtime

Biblioteca SQLite genérica para Java 22+ y GraalVM 25 Native Image.  
**Rust** expone la C ABI completa de SQLite vía `libsqlite3-sys`.  
**Java** la consume con Panama FFI (`java.lang.foreign.*`, JEP 454 — estable desde Java 22) — sin JNI, sin extracción de JARs.

## Arquitectura

```
Java (Panama FFI)
  SqliteConnection  ←  SqliteStatement  ←  SqliteLibrary
       ↓
  libsqlite_native_runtime.{so,dylib}       ← Rust (libsqlite3-sys bundled)
       ↓
  SQLite 3.46 (amalgamation compilada dentro del .so)
```

## Estructura

```
sqlite-native-runtime/
  rust/          Crate Rust — C ABI (cdylib + staticlib)
  java/          Biblioteca Java Maven (Java 22, Panama FFI estable — JEP 454)
    src/main/java/mx/rafex/sqlite/
      SqliteLibrary.java    — bindings de bajo nivel (MethodHandle por símbolo snr_*)
      SqliteConnection.java — conexión de alto nivel (AutoCloseable)
      SqliteStatement.java  — prepared statement (AutoCloseable)
      SqliteException.java  — excepción runtime
```

## Build

### Requisitos

- Rust stable (aarch64-apple-darwin o x86_64-unknown-linux-gnu)
- GraalVM JDK 25 (incluye `native-image`)
- Java 22+ (bytecode target 22; Panama FFM es estable en JEP 454 — sin flags preview)
- Maven 3.9+

```bash
make build        # compila Rust + Java
make test         # build + smoke test
make package      # genera JAR en java/target/
```

## Uso

### Dependencia Maven (instalación local)

```bash
make package
mvn install:install-file \
  -Dfile=sqlite-native-runtime/java/target/sqlite-native-runtime-0.1.0.jar \
  -DgroupId=mx.rafex -DartifactId=sqlite-native-runtime \
  -Dversion=0.1.0 -Dpackaging=jar
```

```xml
<dependency>
  <groupId>mx.rafex</groupId>
  <artifactId>sqlite-native-runtime</artifactId>
  <version>0.1.0</version>
</dependency>
```

### Localización de la librería nativa

Java busca la librería en este orden:

1. Propiedad de sistema `snr.lib`
2. Variable de entorno `SNR_LIB`
3. Rutas por defecto: `/usr/local/lib/`, `/opt/snr/lib/`, directorio de trabajo

```bash
java --enable-native-access=ALL-UNNAMED \
     -Dsnr.lib=/ruta/a/libsqlite_native_runtime.dylib \
     -jar mi-app.jar
```

### API básica

```java
try (var db = SqliteConnection.open("/data/app.db")) {
    db.enableWal().busyTimeout(5000);

    db.exec("CREATE TABLE IF NOT EXISTS items (id INTEGER PRIMARY KEY, name TEXT, val REAL)");

    try (var ins = db.prepare("INSERT INTO items(name, val) VALUES(?, ?)")) {
        ins.bindText(1, "alfa").bindDouble(2, 1.5).stepAndDone();
        ins.reset().bindText(1, "beta").bindDouble(2, 2.5).stepAndDone();
    }

    try (var q = db.prepare("SELECT id, name, val FROM items ORDER BY id")) {
        while (q.step()) {
            long id = q.columnInt(0); String name = q.columnText(1); double val = q.columnDouble(2);
        }
    }

    db.transaction(() -> db.exec("UPDATE items SET val = val * 2"));

    db.withSavepoint("sp1", () -> db.exec("DELETE FROM items WHERE val < 1"));
}
```

### WAL + checkpoint

```java
db.enableWal();                                              // journal_mode=WAL + synchronous=NORMAL
db.walCheckpoint(SqliteConnection.WalMode.TRUNCATE, null);
db.walAutocheckpoint(1000);
```

### Parámetros con nombre

```java
try (var q = db.prepare("SELECT * FROM items WHERE name = :n")) {
    q.bindText(q.parameterIndex(":n"), "alfa");
    while (q.step()) { ... }
}
```

## GraalVM Native Image

Flags necesarios al construir la imagen nativa del proyecto consumidor:

```
--initialize-at-run-time=mx.rafex.sqlite.SqliteLibrary
--enable-native-access=ALL-UNNAMED
```

Para compilar el smoke test como binario nativo (requiere GraalVM 25):

```bash
make native
# o con just:
just native
```

## Funciones C ABI exportadas (`snr_*`)

| Categoría       | Funciones                                                                              |
|-----------------|----------------------------------------------------------------------------------------|
| Conexión        | `snr_open`, `snr_open_memory`, `snr_close`, `snr_ping`, `snr_sqlite_version`          |
| Exec            | `snr_exec`, `snr_last_insert_rowid`, `snr_changes`, `snr_set_busy_timeout`             |
| Statements      | `snr_prepare`, `snr_stmt_close`, `snr_stmt_reset`, `snr_stmt_clear_bindings`           |
| Bind (1-based)  | `snr_bind_null/int/double/text/blob`, `snr_bind_parameter_index`                       |
| Step            | `snr_step` → 1=ROW, 0=DONE, -1=ERROR                                                  |
| Column (0-based)| `snr_column_count/type/int/double/text/text_owned/blob/bytes/name`                    |
| Transacciones   | `snr_begin/begin_immediate/begin_exclusive/commit/rollback`                             |
| Savepoints      | `snr_savepoint/release/rollback_to`                                                    |
| WAL             | `snr_wal_checkpoint`, `snr_wal_autocheckpoint`                                         |
| Errores         | `snr_last_error` (puntero interno, no usar con Loom), `snr_last_error_copy` (copia heap, segura con virtual threads), `snr_free_string` (libera char*) |