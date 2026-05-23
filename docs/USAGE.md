# Guía de uso — sqlite-native-runtime

> Antes de usar la librería asegúrate de haberla instalado correctamente.  
> Ver [INSTALL.md](INSTALL.md).

---

## Abrir una conexión

```java
import mx.rafex.sqlite.SqliteConnection;
import mx.rafex.sqlite.SqliteStatement;

// Base de datos en disco (se crea si no existe)
try (var db = SqliteConnection.open("/ruta/a/mi-base.db")) {
    // ...
}

// Base de datos en memoria (volátil, ideal para tests)
try (var db = SqliteConnection.memory()) {
    // ...
}
```

Ambos métodos son `AutoCloseable` — el bloque `try-with-resources` cierra la conexión al finalizar.

---

## Configuración inicial recomendada

```java
db.enableWal()          // journal_mode=WAL  (lectores y escritores no se bloquean)
  .busyTimeout(5_000);  // reintentar 5 s si la BD está bloqueada
```

---

## DDL — Crear tablas

```java
db.exec("""
    CREATE TABLE IF NOT EXISTS productos (
        id    INTEGER PRIMARY KEY,
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
try (var ins = db.prepare("INSERT INTO productos(nombre, precio, stock) VALUES(?, ?, ?)")) {
    ins.bindText(1, "Teclado")
       .bindDouble(2, 49.99)
       .bindInt(3, 100)
       .stepAndDone();
}
```

### Múltiples filas (batch)

```java
try (var ins = db.prepare("INSERT INTO productos(nombre, precio, stock) VALUES(?, ?, ?)")) {
    String[][] filas = {
        {"Ratón",    "29.99", "200"},
        {"Monitor",  "299.0", "50" },
        {"Auriculares", "89.0", "75"},
    };
    for (var f : filas) {
        ins.bindText(1, f[0])
           .bindDouble(2, Double.parseDouble(f[1]))
           .bindInt(3, Integer.parseInt(f[2]))
           .stepAndDone();
        ins.reset().clearBindings();
    }
}
```

> `reset()` + `clearBindings()` reutiliza el prepared statement sin volver a compilar el SQL.

### ID de la última inserción

```java
long id = db.lastInsertRowid();
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

### Tipos de columna disponibles

| Método | Tipo Java | Tipo SQLite |
|---|---|---|
| `columnInt(i)` | `long` | INTEGER |
| `columnDouble(i)` | `double` | REAL |
| `columnText(i)` | `String` | TEXT |
| `columnBlob(i)` | `byte[]` | BLOB |
| `columnType(i)` | `int` (1-5) | tipo SQLite raw |

> Los índices de columna son **0-based**.

---

## Parámetros nombrados

```java
try (var q = db.prepare("SELECT * FROM productos WHERE nombre = :n AND precio < :max")) {
    q.bindText(q.parameterIndex(":n"),   "Teclado");
    q.bindDouble(q.parameterIndex(":max"), 100.0);
    while (q.step()) { ... }
}
```

---

## Transacciones

```java
// Transacción automática (BEGIN / COMMIT / ROLLBACK en caso de excepción)
db.transaction(() -> {
    try (var u = db.prepare("UPDATE productos SET stock = stock - ? WHERE id = ?")) {
        u.bindInt(1, 5).bindInt(2, 1).stepAndDone();
    }
    try (var ins = db.prepare("INSERT INTO pedidos(producto_id, cantidad) VALUES(?, ?)")) {
        ins.bindInt(1, 1).bindInt(2, 5).stepAndDone();
    }
});
```

### Transacciones manuales

```java
db.beginImmediate();   // o begin() / beginExclusive()
try {
    db.exec("UPDATE productos SET stock = stock - 1 WHERE id = 1");
    db.commit();
} catch (Exception e) {
    db.rollback();
    throw e;
}
```

---

## Savepoints (transacciones anidadas)

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
    db.releaseSavepoint("sp1");   // confirma
} catch (Exception e) {
    db.rollbackToSavepoint("sp1");
    throw e;
}
```

---

## WAL y checkpoint

```java
// Activar WAL (recomendado para escrituras concurrentes)
db.enableWal();

// Checkpoint manual — vuelca el WAL a la BD principal
db.walCheckpoint(SqliteConnection.WalMode.TRUNCATE, null);

// Checkpoint automático cada N páginas (0 = desactivar)
db.walAutocheckpoint(1000);
```

---

## Manejo de errores

Todas las operaciones lanzan `SqliteException` (unchecked) ante errores de SQLite:

```java
import mx.rafex.sqlite.SqliteException;

try (var db = SqliteConnection.open("/ruta/bd.db")) {
    db.exec("INVALID SQL");
} catch (SqliteException e) {
    System.err.println("Error SQLite: " + e.getMessage());
}
```

---

## Uso con virtual threads (Project Loom)

La librería es **segura con virtual threads** con las siguientes consideraciones:

- `snr_last_error_copy()` devuelve una copia del mensaje de error en el heap de Java — segura con Loom.  
  (Evita `snr_last_error()` que devuelve un puntero interno potencialmente inválido si el thread se deschedula.)
- Cada `SqliteConnection` protege sus llamadas con `FULLMUTEX` de SQLite — las llamadas individuales son thread-safe.
- Las **transacciones completas** deben serializarse externamente (`synchronized`, `ReentrantLock`, etc.) porque SQLite no soporta `BEGIN` concurrentes sobre la misma conexión.

```java
// Ejemplo: múltiples virtual threads con una conexión compartida
var db = SqliteConnection.open("shared.db");

try (var executor = Executors.newVirtualThreadPerTaskExecutor()) {
    for (int i = 0; i < 100; i++) {
        final int n = i;
        executor.submit(() -> {
            synchronized (db) {                          // serializa transacciones
                db.transaction(() -> {
                    try (var ins = db.prepare("INSERT INTO log(msg) VALUES(?)")) {
                        ins.bindText(1, "thread-" + n).stepAndDone();
                    }
                });
            }
        });
    }
}
```

---

## GraalVM Native Image — ejemplo completo

```java
// Main.java
public class Main {
    public static void main(String[] args) throws Exception {
        try (var db = SqliteConnection.memory()) {
            db.exec("CREATE TABLE t (x INTEGER)");
            try (var ins = db.prepare("INSERT INTO t VALUES(?)")) {
                for (int i = 1; i <= 5; i++) {
                    ins.bindInt(1, i).stepAndDone();
                    ins.reset();
                }
            }
            try (var q = db.prepare("SELECT sum(x) FROM t")) {
                if (q.step()) System.out.println("Suma: " + q.columnInt(0));
            }
        }
    }
}
```

Compilar como binario nativo:

```sh
native-image \
  --initialize-at-run-time=mx.rafex.sqlite.SqliteLibrary \
  --enable-native-access=ALL-UNNAMED \
  -cp sqlite-native-runtime-0.1.1.jar:. \
  Main

# Ejecutar
SNR_LIB=/usr/local/lib/libsqlite_native_runtime.so ./main
```

---

## API de referencia rápida

### `SqliteConnection`

| Método | Descripción |
|---|---|
| `open(path)` | Abre o crea una BD en disco |
| `memory()` | BD en memoria |
| `exec(sql)` | Ejecuta SQL sin resultado |
| `prepare(sql)` | Devuelve un `SqliteStatement` |
| `transaction(fn)` | BEGIN / COMMIT / ROLLBACK |
| `beginImmediate()` / `commit()` / `rollback()` | Transacción manual |
| `withSavepoint(name, fn)` | Savepoint con rollback automático |
| `enableWal()` | Activa WAL + synchronous=NORMAL |
| `busyTimeout(ms)` | Tiempo de espera en BD bloqueada |
| `walCheckpoint(mode, db)` | Checkpoint manual |
| `walAutocheckpoint(pages)` | Checkpoint automático |
| `lastInsertRowid()` | ID de la última inserción |
| `changes()` | Filas afectadas por la última escritura |
| `sqliteVersion()` | Versión de SQLite embebida |
| `close()` | Cierra la conexión |

### `SqliteStatement`

| Método | Descripción |
|---|---|
| `bindInt(i, v)` | Parámetro INTEGER (1-based) |
| `bindDouble(i, v)` | Parámetro REAL (1-based) |
| `bindText(i, v)` | Parámetro TEXT (1-based) |
| `bindBlob(i, v)` | Parámetro BLOB (1-based) |
| `bindNull(i)` | Parámetro NULL (1-based) |
| `parameterIndex(name)` | Índice de parámetro nombrado |
| `step()` | Avanza al siguiente resultado (`true` = hay fila) |
| `stepAndDone()` | `step()` para escrituras (sin filas) |
| `reset()` | Reinicia el statement (reutilizable) |
| `clearBindings()` | Limpia todos los parámetros |
| `columnInt(i)` | Valor INTEGER de la columna i (0-based) |
| `columnDouble(i)` | Valor REAL de la columna i (0-based) |
| `columnText(i)` | Valor TEXT de la columna i (0-based) |
| `columnBlob(i)` | Valor BLOB de la columna i (0-based) |
| `columnType(i)` | Tipo SQLite de la columna i (0-based) |
| `columnCount()` | Número de columnas en el resultado |
| `columnName(i)` | Nombre de la columna i (0-based) |
| `close()` | Libera el statement |
