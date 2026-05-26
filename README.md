# sqlite-native-runtime

Biblioteca SQLite para Java con bindings nativos compilados en Rust.
Ofrece tres mecanismos de integración según la versión de Java y los requisitos de compilación:

| Binding | Java | Mecanismo | GraalVM native-image | Flag extra |
|---|---|---|---|---|
| **FFM Java 25** | 25+ | Panama FFM estable (JEP 454) | ✅ Soportado | ninguno |
| **JNI Java 21** | 21+ | JNI clásico | ✅ Soportado | ninguno |
| **FFM Java 21** | 21 | Panama FFM preview (JEP 442) | ❌ No soportado | `--enable-preview` |

> **Recomendación:** usa **FFM Java 25** si tu JVM es 25+. Usa **JNI Java 21** si necesitas Java 21 y/o GraalVM native-image.
> FFM Java 21 (preview) es solo para proyectos ya existentes en Java 21 preview — el bytecode con `minor_version=0xFFFF` no es compatible con native-image ni con JVMs 22-24.

---

## Instalación rápida

```sh
curl -sS https://raw.githubusercontent.com/rafex/sqlite-native-runtime/main/scripts/release/install.sh | sh
```

El script instala la librería nativa (`.so`) en `~/.local/lib/` o `/usr/local/lib/`.
Para más opciones consulta la [guía de instalación completa](docs/INSTALL.md).

---

## Uso rápido

### FFM Java 25

```java
import mx.rafex.ether.sqlite.FfmSqliteConnection;
import mx.rafex.ether.sqlite.SqliteConnection;
import mx.rafex.ether.sqlite.SqliteException;

try (SqliteConnection db = FfmSqliteConnection.open("/data/app.db")) {
    db.enableWal().busyTimeout(5_000);

    db.exec("CREATE TABLE IF NOT EXISTS items (id INTEGER PRIMARY KEY, name TEXT)");

    try (var ins = db.prepare("INSERT INTO items(name) VALUES(?)")) {
        ins.bindText(1, "alfa").stepAndDone();
    }

    try (var q = db.prepare("SELECT id, name FROM items")) {
        while (q.step()) {
            System.out.printf("%d  %s%n", q.columnInt(0), q.columnText(1));
        }
    }
}
```

### JNI Java 21

```java
import mx.rafex.ether.sqlite.JniSqliteConnection;
import mx.rafex.ether.sqlite.SqliteConnection;

try (SqliteConnection db = JniSqliteConnection.open("/data/app.db")) {
    // API idéntica a FfmSqliteConnection
    db.enableWal().busyTimeout(5_000);
    // ...
}
```

---

## Arquitectura

```
Java (SqliteConnection interface — mx.rafex.ether.sqlite)
  ├─ FfmSqliteConnection   → libether_sqlite_ffm_runtime.so  (Panama FFM, ABI C snr_*)
  └─ JniSqliteConnection   → libether_sqlite_jni_runtime.so  (JNI Java_mx_rafex_*)
                                      ↓
                             ether-sqlite-core (Rust rlib)
                                      ↓
                             SQLite (amalgamation bundled via libsqlite3-sys)
```

La librería nativa **no requiere** SQLite instalado en el sistema — SQLite 3 está compilado dentro del `.so`.

---

## Estructura del proyecto

```
sqlite-native-runtime/
  sources/
    rust/
      ether-sqlite-core/            ← rlib: lógica SQLite + C ABI (snr_*)
      ether-sqlite-ffm/             ← cdylib: re-exporta core → libether_sqlite_ffm_runtime.so
      ether-sqlite-jni/             ← cdylib: ABI JNI (Java_*) → libether_sqlite_jni_runtime.so
    java/
      ether-sqlite-core/            ← interfaces: SqliteConnection, SqliteStatement, SqliteException
      ether-sqlite-ffm-runtime/     ← Java 25 FFM estable — FfmSqliteConnection
      ether-sqlite-ffm-java21-runtime/ ← Java 21 FFM preview — FfmJava21SqliteConnection (JAR only)
      ether-sqlite-jni-runtime/     ← Java 21 JNI — JniSqliteConnection
```

---

## Guías

- [Instalación](docs/INSTALL.md) — descargar librería, configurar paths, Maven/Gradle, GraalVM
- [Uso y API](docs/USAGE.md) — ejemplos completos, transacciones, WAL, virtual threads, native-image

---

## Artefactos del release

Cada [release de GitHub](https://github.com/rafex/sqlite-native-runtime/releases/latest) incluye:

| Archivo | Descripción |
|---|---|
| `libether_sqlite_ffm_runtime-linux-amd64.so` | Librería FFM — Linux x86\_64 |
| `libether_sqlite_ffm_runtime-linux-arm64.so` | Librería FFM — Linux aarch64 |
| `libether_sqlite_jni_runtime-linux-amd64.so` | Librería JNI — Linux x86\_64 |
| `libether_sqlite_jni_runtime-linux-arm64.so` | Librería JNI — Linux aarch64 |
| `ether-sqlite-ffm-runtime-{v}.jar` | JAR thin FFM Java 25 |
| `ether-sqlite-ffm-runtime-{v}-fat.jar` | JAR fat FFM Java 25 (incluye dependencias) |
| `ether-sqlite-ffm-java21-runtime-{v}-fat.jar` | JAR fat FFM Java 21 preview |
| `ether-sqlite-jni-runtime-{v}-fat.jar` | JAR fat JNI Java 21 |
| `ether-sqlite-ffm-linux-amd64.bin` | Binario nativo FFM — Linux x86\_64 |
| `ether-sqlite-ffm-linux-arm64.bin` | Binario nativo FFM — Linux aarch64 |
| `ether-sqlite-jni-linux-amd64.bin` | Binario nativo JNI — Linux x86\_64 |
| `ether-sqlite-jni-linux-arm64.bin` | Binario nativo JNI — Linux aarch64 |
| `install.sh` | Script de instalación automática |
| `*.sha256` | Checksums SHA256 |

---

## Build desde fuente

### Requisitos

| Herramienta | Versión |
|---|---|
| Rust | stable |
| GraalVM JDK 25 | para compilar FFM runtime y native images |
| JDK 21 | para compilar JNI runtime |
| Maven | 3.9+ |
| `cargo-zigbuild` | para cross-compilar con glibc 2.17+ |

```sh
git clone https://github.com/rafex/sqlite-native-runtime.git
cd sqlite-native-runtime

# Compilar todo (Rust + Java)
just build

# Solo Rust
cd sources/rust && cargo build --workspace --release

# Solo Java (FFM runtime)
./mvnw package -f sources/java/ether-sqlite-ffm-runtime/pom.xml

# Solo Java (JNI runtime)
./mvnw package -f sources/java/ether-sqlite-jni-runtime/pom.xml
```
