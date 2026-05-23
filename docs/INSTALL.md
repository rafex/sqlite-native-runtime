# Instalación — sqlite-native-runtime

## Requisitos previos

| Requisito | Versión | Notas |
|---|---|---|
| **Java** | **21 exactamente** | Panama FFM era Preview en Java 21 (JEP 442). **Requiere `--enable-preview`**. |
| **OS** | Linux x86\_64 / arm64 **64-bit** | Binarios pre-compilados disponibles. macOS: compilar desde fuente. |
| `curl` o `wget` | cualquiera | Necesario para el script de instalación. |

> ⚠️ **Java 22+ no es compatible.** El bytecode incluye el flag de preview de Java 21
> (`minor_version=0xFFFF`). Las JVMs de Java 22, 23, 24 y 25 lo rechazarán con
> `UnsupportedClassVersionError`. Esta limitación se eliminará en una versión futura
> con soporte JNI.

> ⚠️ **`--enable-preview` es obligatorio en runtime.** Toda aplicación que use esta
> librería debe arrancarse con el flag `--enable-preview`.

La librería nativa **no requiere** SQLite instalado en el sistema — SQLite 3 está compilado dentro del `.so` (amalgamation bundled via `libsqlite3-sys`).

---

## Soporte Raspberry Pi

| Modelo | CPU | OS 64-bit (aarch64) | OS 32-bit (armhf) |
|---|---|---|---|
| Raspberry Pi 3B | Cortex-A53 (ARMv8.0) | ✅ Soportado | ❌ No soportado |
| Raspberry Pi 4B | Cortex-A72 (ARMv8.2) | ✅ Soportado | ❌ No soportado |

**¿Por qué no se soporta arm32 (OS 32-bit)?**  
Java 22+ (requerido por Panama FFM / JEP 454) **no publica builds para arm32**. Los distribuidores principales (Eclipse Temurin, Azul Zulu, GraalVM) abandonaron arm32 a partir de Java 22. Sin JDK no hay forma de cargar la librería nativa.

**Raspberry Pi 3B y 4B con OS 64-bit funcionan sin configuración adicional:**

```sh
# 1. Instalar un OS 64-bit en tu Raspberry Pi:
#    Raspberry Pi OS 64-bit  → https://www.raspberrypi.com/software/
#    Ubuntu Server 24.04 LTS → https://ubuntu.com/download/raspberry-pi

# 2. Instalar la librería (detecta aarch64 automáticamente):
curl -sS https://raw.githubusercontent.com/rafex/sqlite-native-runtime/main/scripts/release/install.sh | sh

# 3. Ejecutar tu aplicación:
java --enable-preview --enable-native-access=ALL-UNNAMED -jar mi-app.jar
```

> Si ejecutas `install.sh` en un OS 32-bit recibirás un mensaje de error claro  
> con instrucciones para migrar a 64-bit.

---

## Instalación rápida (recomendado)

> ⚠️ **Requisito:** Java 21 con `--enable-preview`. Ver [Requisitos previos](#requisitos-previos).

```sh
curl -sS https://raw.githubusercontent.com/rafex/sqlite-native-runtime/main/scripts/release/install.sh | sh
```

El script:
1. Detecta OS y arquitectura (`x86_64` / `aarch64`)
2. Consulta la última versión en la API de GitHub
3. Descarga `libsqlite_native_runtime-linux-{arch}.so` del release
4. Verifica el SHA256
5. Instala la librería:

| Condición | Destino | Auto-detectado por la JVM |
|---|---|---|
| `sudo` disponible | `/usr/local/lib/libsqlite_native_runtime.so` | ✅ sí |
| Sin `sudo` | `~/.local/lib/libsqlite_native_runtime.so` | ✅ sí (≥ v0.1.1) |

Si se instala sin `sudo`, el script también añade `export SNR_LIB=...` a `~/.bashrc` / `~/.zshrc` para compatibilidad con versiones anteriores.

### Opciones del script

```sh
# Instalar una versión específica
SNR_VERSION=v0.1.1 curl -sS ...install.sh | sh

# Forzar instalación en directorio de usuario (aunque sudo esté disponible)
SNR_USER_INSTALL=1 curl -sS ...install.sh | sh
```

---

## Instalación manual

### Descargar desde GitHub Releases

Ve a [Releases](https://github.com/rafex/sqlite-native-runtime/releases) y descarga el artefacto para tu arquitectura:

| Archivo | Arquitectura |
|---|---|
| `libsqlite_native_runtime-linux-amd64.so` | Linux x86\_64 |
| `libsqlite_native_runtime-linux-arm64.so` | Linux aarch64 |

Verifica el SHA256:

```sh
sha256sum -c libsqlite_native_runtime-linux-amd64.so.sha256
```

### Instalar en el sistema (con sudo)

```sh
sudo cp libsqlite_native_runtime-linux-amd64.so /usr/local/lib/libsqlite_native_runtime.so
sudo chmod 755 /usr/local/lib/libsqlite_native_runtime.so
sudo ldconfig
```

La JVM detecta automáticamente `/usr/local/lib/` — no necesitas configurar nada más.

### Instalar como usuario (sin sudo)

```sh
mkdir -p ~/.local/lib
cp libsqlite_native_runtime-linux-amd64.so ~/.local/lib/libsqlite_native_runtime.so
chmod 755 ~/.local/lib/libsqlite_native_runtime.so
```

La JVM detecta automáticamente `~/.local/lib/` desde la versión **v0.1.1**.  
Para versiones anteriores, o para ser explícito, añade a tu shell:

```sh
export SNR_LIB="$HOME/.local/lib/libsqlite_native_runtime.so"
```

---

## Rutas de búsqueda de la librería

La JVM busca la librería en este orden:

| Prioridad | Ruta / Mecanismo |
|---|---|
| 1 | Propiedad de sistema: `-Dsnr.lib=/ruta/completa.so` |
| 2 | Variable de entorno: `SNR_LIB=/ruta/completa.so` |
| 3 | `~/.local/lib/libsqlite_native_runtime.so` *(instalación usuario)* |
| 4 | `/usr/local/lib/libsqlite_native_runtime.so` *(instalación sistema Linux)* |
| 5 | `/opt/snr/lib/libsqlite_native_runtime.so` |
| 6 | `/usr/local/lib/libsqlite_native_runtime.dylib` *(macOS sistema)* |
| 7 | `/opt/homebrew/lib/libsqlite_native_runtime.dylib` *(Homebrew macOS)* |
| 8 | Directorio de trabajo *(solo desarrollo, imprime aviso)* |

---

## macOS — Compilar desde fuente

Los binarios pre-compilados en los releases son solo para Linux. En macOS compila localmente:

```sh
# Requisitos: Rust stable, GraalVM JDK 25
git clone https://github.com/rafex/sqlite-native-runtime.git
cd sqlite-native-runtime
make build-rust
# Librería: sqlite-native-runtime/rust/target/release/libsqlite_native_runtime.dylib
```

Instala con:
```sh
sudo cp sqlite-native-runtime/rust/target/release/libsqlite_native_runtime.dylib \
        /usr/local/lib/
```

O con Homebrew prefix:
```sh
cp sqlite-native-runtime/rust/target/release/libsqlite_native_runtime.dylib \
   /opt/homebrew/lib/
```

---

## Integración con Maven / Gradle

### Instalar el JAR en el repositorio local

```sh
# Descarga sqlite-native-runtime-{version}.jar del release y:
mvn install:install-file \
  -Dfile=sqlite-native-runtime-0.1.1.jar \
  -DgroupId=mx.rafex \
  -DartifactId=sqlite-native-runtime \
  -Dversion=0.1.1 \
  -Dpackaging=jar
```

### `pom.xml`

```xml
<dependency>
  <groupId>mx.rafex</groupId>
  <artifactId>sqlite-native-runtime</artifactId>
  <version>0.1.1</version>
</dependency>
```

### `build.gradle` / `build.gradle.kts`

```kotlin
dependencies {
    implementation("mx.rafex:sqlite-native-runtime:0.1.1")
}
```

### Flags de JVM obligatorios

Esta librería usa Panama FFM en modo **preview de Java 21** (JEP 442).
**Ambos flags son obligatorios** en cualquier aplicación que la use:

```
--enable-preview
--enable-native-access=ALL-UNNAMED
```

**Maven Compiler Plugin** (si tu proyecto también compila con Java 21 preview):
```xml
<plugin>
  <artifactId>maven-compiler-plugin</artifactId>
  <configuration>
    <release>21</release>
    <compilerArgs>
      <arg>--enable-preview</arg>
    </compilerArgs>
  </configuration>
</plugin>
```

**Maven Surefire** (tests):
```xml
<configuration>
  <argLine>--enable-preview --enable-native-access=ALL-UNNAMED</argLine>
</configuration>
```

**Spring Boot** (`application.properties`):
```properties
spring.jvm.arguments=--enable-preview --enable-native-access=ALL-UNNAMED
```

**Spring Boot** (Maven, `pom.xml`):
```xml
<plugin>
  <groupId>org.springframework.boot</groupId>
  <artifactId>spring-boot-maven-plugin</artifactId>
  <configuration>
    <jvmArguments>--enable-preview --enable-native-access=ALL-UNNAMED</jvmArguments>
  </configuration>
</plugin>
```

**Línea de comandos**:
```sh
java --enable-preview --enable-native-access=ALL-UNNAMED -jar mi-app.jar
```

---

## GraalVM Native Image

Si compilas tu aplicación como Native Image, añade estos flags al plugin:

```xml
<plugin>
  <groupId>org.graalvm.buildtools</groupId>
  <artifactId>native-maven-plugin</artifactId>
  <configuration>
    <buildArgs>
      <!-- SqliteLibrary carga la .so en el bloque static: diferir a runtime -->
      <buildArg>--initialize-at-run-time=mx.rafex.sqlite.SqliteLibrary</buildArg>
      <buildArg>--enable-native-access=ALL-UNNAMED</buildArg>
      <!-- Bytecode Java 21 preview: el compilador AOT también necesita el flag -->
      <buildArg>--enable-preview</buildArg>
    </buildArgs>
  </configuration>
</plugin>
```

En tiempo de ejecución del binario nativo, la librería debe estar instalada o apuntar vía `SNR_LIB`:

```sh
SNR_LIB=/usr/local/lib/libsqlite_native_runtime.so ./mi-binario-nativo
```

---

## Verificar la instalación

```sh
# Comprueba que la librería es válida y exporta los símbolos snr_*
nm -D /usr/local/lib/libsqlite_native_runtime.so | grep snr_open
# Debe mostrar: ... T snr_open
```
