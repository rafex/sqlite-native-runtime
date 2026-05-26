# Guía de uso — sqlite-native-runtime

> Antes de comenzar asegúrate de tener la librería nativa instalada.
> Ver [INSTALL.md](INSTALL.md).

---

## Imports por binding

### FFM Java 25 (recomendado para Java 25+)

```java
import mx.rafex.ether.sqlite.FfmSqliteConnection;
import mx.rafex.ether.sqlite.SqliteConnection;
import mx.rafex.ether.sqlite.SqliteStatement;
import mx.rafex.ether.sqlite.SqliteException;
```

### JNI Java 21 (recomendado para Java 21+ y native-image)

```java
import mx.rafex.ether.sqlite.JniSqliteConnection;
import mx.rafex.ether.sqlite.SqliteConnection;
import mx.rafex.ether.sqlite.SqliteStatement;
import mx.rafex.ether.sqlite.SqliteException;
```

### FFM Java 21 preview (solo JAR, sin native-image)

```java
import mx.rafex.ether.sqlite.FfmJava21SqliteConnection;
import mx.rafex.ether.sqlite.SqliteConnection;
import mx.rafex.ether.sqlite.SqliteStatement;
import mx.rafex.ether.sqlite.SqliteException;
```

> Los tres bindings implementan la misma interfaz `SqliteConnection`. Los ejemplos de esta guía
> usan `FfmSqliteConnection` — sustitúyelo por `JniSqliteConnection` o `FfmJava21SqliteConnection`
> para los otros bindings; la API es idéntica.

---

## Abrir una conexión

```java
// Base de datos en disco (se crea si no existe)
try (SqliteConnection db = FfmSqliteConnection.open("/ruta/a/mi-base.db")) {
    // operaciones...
}

// Base de datos en memoria (volátil, ideal para tests)
try (SqliteConnection db = FfmSqliteConnection.memory()) {
    // operaciones...
}

// JNI — mismo patrón
try (SqliteConnection db = JniSqliteConnection.open("/ruta/a/mi-base.db")) {
    // operaciones...
}
```

Ambos métodos son `AutoCloseable` — el bloque `try-with-resources` cierra la conexión automáticamente.

---

## Configuración inicial recomendada

```java
db.enableWal()          // journal_mode=WAL (lectores y escritores no se bloquean)
  .busyTimeout(5_000);  // reintentar 5 s si la BD está bloqueada
```

---

## DDL — Crear tablas

```java
db.exec("""
    CREATE TABLE IF NOT EXISTS productos (
        id     INTEGER PRIMARY KEY,
        nombre TEXT    NOT NULL,
        precio REAL    NOT NULL,
        stock  INTEGER DEFAULT 0
    )
""");
```

---

## Insertar datos

### Una fila

```java
try (SqliteStatement ins = db.prepare("INSERT INTO productos(nombre, precio, stock) VALUES(?, ?, ?)")) {
    ins.bindText(1, "Teclado")
       .bindDouble(2, 49.99)
       .bindInt(3, 100)
       .stepAndDone();
}
```

### Múltiples filas (batch)

```java
try (var ins = db.prepare("INSERT INTO productos(nombre, precio, stock) VALUES(?, ?, ?)")) {
    record Producto(String nombre, double precio, int stock) {}
    var lista = List.of(
        new Producto("Ratón",       29.99, 200),
        new Producto("Monitor",    299.00,  50),
        new Producto("Auriculares", 89.00,  75)
    );
    for (var p : lista) {
        ins.bindText(1, p.nombre())
           .bindDouble(2, p.precio())
           .bindInt(3, p.stock())
           .stepAndDone();
        ins.reset().clearBindings();   // reutiliza el statement sin recompilar SQL
    }
}
```

### ID de la última inserción y filas afectadas

```java
long id     = db.lastInsertRowid();
long cambios = db.changes();
```

---

## Consultar datos

```java
try (var q = db.prepare("SELECT id, nombre, precio, stock FROM productos ORDER BY precio")) {
    while (q.step()) {
        long   id     = q.columnInt(0);
        String nombre = q.columnText(1);
        double precio = q.columnDouble(2);
        long   stock  = q.columnInt(3);
        System.out.printf("%d  %-20s  %.2f  %d%n", id, nombre, precio, stock);
    }
}
```

### Tipos de columna

| Método | Tipo Java | Tipo SQLite | Índice |
|---|---|---|---|
| `columnInt(i)` | `long` | INTEGER | 0-based |
| `columnDouble(i)` | `double` | REAL | 0-based |
| `columnText(i)` | `String` | TEXT | 0-based |
| `columnBlob(i)` | `byte[]` | BLOB | 0-based |
| `columnType(i)` | `int` (1-5) | tipo SQLite raw | 0-based |

Constantes de tipo: `SqliteStatement.TYPE_INTEGER=1`, `TYPE_FLOAT=2`, `TYPE_TEXT=3`, `TYPE_BLOB=4`, `TYPE_NULL=5`.

### Parámetros nombrados

```java
try (var q = db.prepare("SELECT * FROM productos WHERE nombre = :n AND precio < :max")) {
    q.bindText(q.parameterIndex(":n"),    "Teclado");
    q.bindDouble(q.parameterIndex(":max"), 100.0);
    while (q.step()) {
        // ...
    }
}
```

### Verificar tipo de columna antes de leer

```java
try (var q = db.prepare("SELECT valor FROM datos")) {
    while (q.step()) {
        int tipo = q.columnType(0);
        switch (tipo) {
            case SqliteStatement.TYPE_INTEGER -> System.out.println(q.columnInt(0));
            case SqliteStatement.TYPE_FLOAT   -> System.out.println(q.columnDouble(0));
            case SqliteStatement.TYPE_TEXT    -> System.out.println(q.columnText(0));
            case SqliteStatement.TYPE_BLOB    -> System.out.println(Arrays.toString(q.columnBlob(0)));
            case SqliteStatement.TYPE_NULL    -> System.out.println("NULL");
        }
    }
}
```

---

## Transacciones

### Transacción automática (recomendada)

```java
// BEGIN / COMMIT automático — ROLLBACK si lanza excepción
db.transaction(() -> {
    try (var u = db.prepare("UPDATE productos SET stock = stock - ? WHERE id = ?")) {
        u.bindInt(1, 5).bindInt(2, 1).stepAndDone();
    }
    try (var ins = db.prepare("INSERT INTO pedidos(producto_id, cantidad) VALUES(?, ?)")) {
        ins.bindInt(1, 1).bindInt(2, 5).stepAndDone();
    }
});
```

### Transacción inmediata (evita conflictos de escritura concurrente)

```java
db.transactionImmediate(() -> {
    db.exec("UPDATE stock SET cantidad = cantidad - 1 WHERE sku = 'ABC'");
});
```

### Transacción manual

```java
db.beginImmediate();   // o begin() / beginExclusive()
try {
    db.exec("UPDATE productos SET precio = precio * 1.1");
    db.commit();
} catch (Exception e) {
    db.rollback();
    throw e;
}
```

---

## Savepoints (transacciones anidadas)

### Automático

```java
db.withSavepoint("sp_ajuste", () -> {
    db.exec("UPDATE productos SET precio = precio * 1.1");
    // Si lanza excepción → ROLLBACK TO sp_ajuste automático
});
```

### Manual

```java
db.savepoint("sp1");
try {
    db.exec("DELETE FROM productos WHERE stock = 0");
    db.release("sp1");     // confirma el savepoint
} catch (Exception e) {
    db.rollbackTo("sp1");  // revierte al savepoint
    throw e;
}
```

---

## WAL y checkpoint

```java
// Activar WAL (recomendado para escrituras concurrentes)
db.enableWal();

// Checkpoint manual — vuelca el WAL a la BD principal
SqliteConnection.WalCheckpointResult r = db.walCheckpoint(SqliteConnection.WalMode.TRUNCATE, null);
System.out.printf("WAL frames: %d, checkpointed: %d%n", r.walFrames(), r.checkpointed());

// Checkpoint automático cada N frames (0 = desactivar)
db.walAutocheckpoint(1000);
```

Modos de checkpoint: `PASSIVE`, `FULL`, `RESTART`, `TRUNCATE`.

---

## Manejo de errores

Todas las operaciones lanzan `SqliteException` (unchecked) ante errores de SQLite:

```java
import mx.rafex.ether.sqlite.SqliteException;

try (var db = FfmSqliteConnection.open("/ruta/bd.db")) {
    db.exec("SQL INVALIDO");
} catch (SqliteException e) {
    System.err.println("Error SQLite: " + e.getMessage());
}
```

---

## Virtual Threads (Project Loom)

La librería es **compatible con virtual threads** con las siguientes consideraciones:

- Cada `SqliteConnection` usa `FULLMUTEX` — las llamadas individuales son thread-safe.
- Las **transacciones completas** (BEGIN → COMMIT/ROLLBACK) deben serializarse externamente,
  ya que SQLite no permite `BEGIN` concurrentes sobre la misma conexión.
- Para concurrencia real usa **una conexión por thread/tarea**.

```java
// Patrón recomendado: una conexión por virtual thread (thread-local o pool)
try (var executor = Executors.newVirtualThreadPerTaskExecutor()) {
    for (int i = 0; i < 100; i++) {
        final int n = i;
        executor.submit(() -> {
            // Cada tarea abre y cierra su propia conexión
            try (var db = FfmSqliteConnection.open("app.db")) {
                db.transaction(() -> {
                    try (var ins = db.prepare("INSERT INTO log(msg) VALUES(?)")) {
                        ins.bindText(1, "tarea-" + n).stepAndDone();
                    }
                });
            }
        });
    }
}
```

---

## GraalVM Native Image

### FFM Java 25 — `pom.xml`

```xml
<plugin>
  <groupId>org.graalvm.buildtools</groupId>
  <artifactId>native-maven-plugin</artifactId>
  <version>0.10.6</version>
  <executions>
    <execution>
      <id>build-native</id>
      <goals><goal>compile-no-fork</goal></goals>
      <phase>package</phase>
    </execution>
  </executions>
  <configuration>
    <buildArgs>
      <buildArg>--initialize-at-run-time=mx.rafex.ether.sqlite.SqliteLibrary</buildArg>
      <buildArg>--enable-native-access=ALL-UNNAMED</buildArg>
    </buildArgs>
  </configuration>
</plugin>
```

Ejecutar el binario nativo:

```sh
# La librería debe estar instalada o apuntar con la variable de entorno
ETHER_SQLITE_LIB=/usr/local/lib/libether_sqlite_ffm_runtime.so ./mi-aplicacion
```

### JNI Java 21 — `pom.xml`

JNI no necesita flags especiales de native-image:

```xml
<plugin>
  <groupId>org.graalvm.buildtools</groupId>
  <artifactId>native-maven-plugin</artifactId>
  <version>0.10.6</version>
  <executions>
    <execution>
      <id>build-native</id>
      <goals><goal>compile-no-fork</goal></goals>
      <phase>package</phase>
    </execution>
  </executions>
</plugin>
```

Ejecutar el binario nativo:

```sh
ETHER_SQLITE_JNI_LIB=/usr/local/lib/libether_sqlite_jni_runtime.so ./mi-aplicacion
```

### Ejemplo completo (FFM Java 25)

```java
// Main.java
import mx.rafex.ether.sqlite.FfmSqliteConnection;
import mx.rafex.ether.sqlite.SqliteConnection;

public class Main {
    public static void main(String[] args) throws Exception {
        try (SqliteConnection db = FfmSqliteConnection.memory()) {
            db.exec("CREATE TABLE numeros (n INTEGER)");

            db.transaction(() -> {
                try (var ins = db.prepare("INSERT INTO numeros VALUES(?)")) {
                    for (int i = 1; i <= 10; i++) {
                        ins.bindInt(1, i).stepAndDone();
                        ins.reset();
                    }
                }
            });

            try (var q = db.prepare("SELECT sum(n), count(n) FROM numeros")) {
                if (q.step()) {
                    System.out.printf("Suma: %d, Filas: %d%n", q.columnInt(0), q.columnInt(1));
                }
            }
        }
    }
}
```

Compilar y ejecutar como binario nativo:

```sh
# Compilar
mvn -Pnative package

# Ejecutar (librería instalada en /usr/local/lib)
./target/mi-aplicacion
```

---

## Usar los binarios nativos precompilados del release

Los releases incluyen binarios nativos listos para ejecutar (Linux x86\_64 / arm64):

```sh
# Descargar desde GitHub Releases
curl -LO https://github.com/rafex/sqlite-native-runtime/releases/latest/download/ether-sqlite-ffm-linux-amd64.bin
chmod +x ether-sqlite-ffm-linux-amd64.bin

# Ejecutar (la librería debe estar instalada)
ETHER_SQLITE_LIB=/usr/local/lib/libether_sqlite_ffm_runtime.so ./ether-sqlite-ffm-linux-amd64.bin

# O con JNI
curl -LO .../ether-sqlite-jni-linux-amd64.bin
chmod +x ether-sqlite-jni-linux-amd64.bin
ETHER_SQLITE_JNI_LIB=/usr/local/lib/libether_sqlite_jni_runtime.so ./ether-sqlite-jni-linux-amd64.bin
```

---

## API de referencia rápida

### `SqliteConnection` (interface)

| Método | Descripción |
|---|---|
| `FfmSqliteConnection.open(path)` | Abre o crea una BD en disco (FFM) |
| `FfmSqliteConnection.memory()` | BD en memoria (FFM) |
| `JniSqliteConnection.open(path)` | Abre o crea una BD en disco (JNI) |
| `JniSqliteConnection.memory()` | BD en memoria (JNI) |
| `exec(sql)` | Ejecuta SQL sin resultado |
| `prepare(sql)` | Devuelve un `SqliteStatement` |
| `transaction(fn)` | BEGIN DEFERRED / COMMIT / ROLLBACK automático |
| `transactionImmediate(fn)` | BEGIN IMMEDIATE / COMMIT / ROLLBACK automático |
| `begin()` / `beginImmediate()` / `beginExclusive()` | Iniciar transacción manual |
| `commit()` / `rollback()` | Confirmar / revertir transacción manual |
| `withSavepoint(name, fn)` | Savepoint con rollback automático si hay excepción |
| `savepoint(name)` / `release(name)` / `rollbackTo(name)` | Savepoints manuales |
| `enableWal()` | Activa WAL + synchronous=NORMAL |
| `busyTimeout(ms)` | Tiempo de espera si la BD está bloqueada |
| `walCheckpoint(mode, dbName)` | Checkpoint manual |
| `walAutocheckpoint(n)` | Checkpoint automático cada `n` frames |
| `lastInsertRowid()` | ID de la última inserción exitosa |
| `changes()` | Filas afectadas por la última operación DML |
| `ping()` | Verifica que la conexión responde |
| `close()` | Cierra la conexión |

### `SqliteStatement` (interface)

| Método | Descripción |
|---|---|
| `bindNull(i)` | Parámetro NULL (1-based) |
| `bindInt(i, v)` | Parámetro INTEGER (1-based) — acepta `long` o `int` |
| `bindDouble(i, v)` | Parámetro REAL (1-based) |
| `bindText(i, v)` | Parámetro TEXT (1-based) |
| `bindBlob(i, data)` | Parámetro BLOB (1-based) |
| `parameterIndex(name)` | Índice (1-based) del parámetro nombrado (`:name`, `?NNN`, `@name`) |
| `step()` | Avanza al siguiente resultado — `true` = hay fila |
| `stepAndDone()` | Ejecuta un paso para escrituras (sin filas de resultado) |
| `reset()` | Reinicia el statement (mantiene bindings) |
| `clearBindings()` | Limpia todos los parámetros a NULL |
| `columnCount()` | Número de columnas en el resultado |
| `columnType(i)` | Tipo SQLite de la columna `i` (0-based) |
| `columnInt(i)` | Valor como `long` (0-based) |
| `columnDouble(i)` | Valor como `double` (0-based) |
| `columnText(i)` | Valor como `String` (0-based), `null` si NULL |
| `columnBlob(i)` | Valor como `byte[]` (0-based), `null` si NULL |
| `columnBytes(i)` | Tamaño en bytes del valor TEXT o BLOB (0-based) |
| `columnName(i)` | Nombre de la columna `i` (0-based) |
| `close()` | Libera el statement |

### `SqliteConnection.WalMode`

| Valor | Comportamiento |
|---|---|
| `PASSIVE` | Solo checkpointea frames no usados por lectores activos |
| `FULL` | Espera a que los lectores terminen, luego checkpointea todo |
| `RESTART` | Como FULL, pero también reinicia el writer del WAL |
| `TRUNCATE` | Como RESTART, pero trunca el WAL a cero bytes al terminar |
