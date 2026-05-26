# Instalación — sqlite-native-runtime

## Elegir el binding adecuado

| Binding | Java | GraalVM native-image | JVM flags extra | Librería nativa |
|---|---|---|---|---|
| **FFM Java 25** | 25+ | ✅ Sí | ninguno | `libether_sqlite_ffm_runtime.so` |
| **JNI Java 21** | 21+ | ✅ Sí | ninguno | `libether_sqlite_jni_runtime.so` |
| **FFM Java 21** | 21 exactamente | ❌ No | `--enable-preview --enable-native-access=ALL-UNNAMED` | `libether_sqlite_ffm_runtime.so` |

> La librería nativa **no requiere** SQLite instalado en el sistema — SQLite 3 está compilado
> dentro del `.so` (amalgamation bundled via `libsqlite3-sys`).

---

## Instalación rápida (recomendada)

```sh
curl -sS https://raw.githubusercontent.com/rafex/sqlite-native-runtime/main/scripts/release/install.sh | sh
```

El script:
1. Detecta OS y arquitectura (`x86_64` / `aarch64`)
2. Descarga la última versión de GitHub Releases
3. Verifica el SHA256
4. Instala **ambas** librerías (FFM + JNI):

| Condición | Destino |
|---|---|
| `sudo` disponible | `/usr/local/lib/` |
| Sin `sudo` | `~/.local/lib/` |

### Opciones del script

```sh
# Versión específica
SNR_VERSION=v0.4.0 curl -sS .../install.sh | sh

# Forzar instalación en directorio de usuario
SNR_USER_INSTALL=1 curl -sS .../install.sh | sh
```

---

## Instalación manual de la librería nativa

### 1. Descargar desde GitHub Releases

Ve a [Releases](https://github.com/rafex/sqlite-native-runtime/releases/latest) y descarga el
artefacto para tu binding y arquitectura:

| Archivo | Binding | Arquitectura |
|---|---|---|
| `libether_sqlite_ffm_runtime-linux-amd64.so` | FFM (Java 25 / Java 21) | Linux x86\_64 |
| `libether_sqlite_ffm_runtime-linux-arm64.so` | FFM (Java 25 / Java 21) | Linux aarch64 |
| `libether_sqlite_jni_runtime-linux-amd64.so` | JNI (Java 21) | Linux x86\_64 |
| `libether_sqlite_jni_runtime-linux-arm64.so` | JNI (Java 21) | Linux aarch64 |

Verifica el SHA256:

```sh
sha256sum -c libether_sqlite_ffm_runtime-linux-amd64.so.sha256
sha256sum -c libether_sqlite_jni_runtime-linux-amd64.so.sha256
```

### 2. Instalar en el sistema (con sudo)

```sh
# FFM
sudo cp libether_sqlite_ffm_runtime-linux-amd64.so /usr/local/lib/libether_sqlite_ffm_runtime.so
sudo chmod 755 /usr/local/lib/libether_sqlite_ffm_runtime.so

# JNI
sudo cp libether_sqlite_jni_runtime-linux-amd64.so /usr/local/lib/libether_sqlite_jni_runtime.so
sudo chmod 755 /usr/local/lib/libether_sqlite_jni_runtime.so

sudo ldconfig
```

### 3. Instalar como usuario (sin sudo)

```sh
mkdir -p ~/.local/lib

# FFM
cp libether_sqlite_ffm_runtime-linux-amd64.so ~/.local/lib/libether_sqlite_ffm_runtime.so
chmod 755 ~/.local/lib/libether_sqlite_ffm_runtime.so

# JNI
cp libether_sqlite_jni_runtime-linux-amd64.so ~/.local/lib/libether_sqlite_jni_runtime.so
chmod 755 ~/.local/lib/libether_sqlite_jni_runtime.so
```

---

## Rutas de búsqueda de la librería

### FFM (`libether_sqlite_ffm_runtime`)

La librería se busca en este orden:

| Prioridad | Mecanismo |
|---|---|
| 1 | Propiedad de sistema: `-Dether.sqlite.lib=/ruta/completa.so` |
| 2 | Variable de entorno: `ETHER_SQLITE_LIB=/ruta/completa.so` |
| 3 | `~/.local/lib/libether_sqlite_ffm_runtime.{dylib,so}` |
| 4 | `/usr/local/lib/libether_sqlite_ffm_runtime.{dylib,so}` |
| 5 | `/opt/snr/lib/libether_sqlite_ffm_runtime.{dylib,so}` |
| 6 | `/opt/homebrew/lib/libether_sqlite_ffm_runtime.{dylib,so}` |
| 7 | Directorio de trabajo *(solo desarrollo, imprime aviso)* |

### JNI (`libether_sqlite_jni_runtime`)

| Prioridad | Mecanismo |
|---|---|
| 1 | Propiedad de sistema: `-Dether.sqlite.jni.lib=/ruta/completa.so` |
| 2 | Variable de entorno: `ETHER_SQLITE_JNI_LIB=/ruta/completa.so` |
| 3 | `~/.local/lib/libether_sqlite_jni_runtime.{dylib,so}` |
| 4 | `/usr/local/lib/libether_sqlite_jni_runtime.{dylib,so}` |
| 5 | `/opt/snr/lib/libether_sqlite_jni_runtime.{dylib,so}` |
| 6 | `/opt/homebrew/lib/libether_sqlite_jni_runtime.{dylib,so}` |
| 7 | Directorio de trabajo *(solo desarrollo, imprime aviso)* |

---

## Integración Maven

### Paso 1 — Instalar el JAR en el repositorio local

Descarga el fat JAR del binding que necesites desde [GitHub Releases](https://github.com/rafex/sqlite-native-runtime/releases/latest):

```sh
# FFM Java 25
mvn install:install-file \
  -Dfile=ether-sqlite-ffm-runtime-{version}-fat.jar \
  -DgroupId=mx.rafex.ether \
  -DartifactId=ether-sqlite-ffm-runtime \
  -Dversion={version} \
  -Dpackaging=jar

# JNI Java 21
mvn install:install-file \
  -Dfile=ether-sqlite-jni-runtime-{version}-fat.jar \
  -DgroupId=mx.rafex.ether \
  -DartifactId=ether-sqlite-jni-runtime \
  -Dversion={version} \
  -Dpackaging=jar

# FFM Java 21 preview
mvn install:install-file \
  -Dfile=ether-sqlite-ffm-java21-runtime-{version}-fat.jar \
  -DgroupId=mx.rafex.ether \
  -DartifactId=ether-sqlite-ffm-java21-runtime \
  -Dversion={version} \
  -Dpackaging=jar
```

### Paso 2 — `pom.xml`

#### FFM Java 25 (recomendado para Java 25+)

```xml
<properties>
  <maven.compiler.release>25</maven.compiler.release>
</properties>

<dependencies>
  <dependency>
    <groupId>mx.rafex.ether</groupId>
    <artifactId>ether-sqlite-ffm-runtime</artifactId>
    <version>{version}</version>
  </dependency>
</dependencies>
```

Surefire (tests):

```xml
<plugin>
  <artifactId>maven-surefire-plugin</artifactId>
  <configuration>
    <argLine>--enable-native-access=ALL-UNNAMED</argLine>
    <environmentVariables>
      <ETHER_SQLITE_LIB>/ruta/a/libether_sqlite_ffm_runtime.so</ETHER_SQLITE_LIB>
    </environmentVariables>
  </configuration>
</plugin>
```

#### JNI Java 21 (recomendado para Java 21+)

```xml
<properties>
  <maven.compiler.release>21</maven.compiler.release>
</properties>

<dependencies>
  <dependency>
    <groupId>mx.rafex.ether</groupId>
    <artifactId>ether-sqlite-jni-runtime</artifactId>
    <version>{version}</version>
  </dependency>
</dependencies>
```

Surefire (tests):

```xml
<plugin>
  <artifactId>maven-surefire-plugin</artifactId>
  <configuration>
    <environmentVariables>
      <ETHER_SQLITE_JNI_LIB>/ruta/a/libether_sqlite_jni_runtime.so</ETHER_SQLITE_JNI_LIB>
    </environmentVariables>
  </configuration>
</plugin>
```

#### FFM Java 21 preview (solo JAR, sin native-image)

```xml
<properties>
  <maven.compiler.release>21</maven.compiler.release>
</properties>

<dependencies>
  <dependency>
    <groupId>mx.rafex.ether</groupId>
    <artifactId>ether-sqlite-ffm-java21-runtime</artifactId>
    <version>{version}</version>
  </dependency>
</dependencies>
```

Compiler plugin (Java 21 preview):

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

Surefire (tests):

```xml
<plugin>
  <artifactId>maven-surefire-plugin</artifactId>
  <configuration>
    <argLine>--enable-preview --enable-native-access=ALL-UNNAMED</argLine>
    <environmentVariables>
      <ETHER_SQLITE_LIB>/ruta/a/libether_sqlite_ffm_runtime.so</ETHER_SQLITE_LIB>
    </environmentVariables>
  </configuration>
</plugin>
```

---

## Integración Gradle

### FFM Java 25

```kotlin
java {
    toolchain { languageVersion = JavaLanguageVersion.of(25) }
}

dependencies {
    implementation("mx.rafex.ether:ether-sqlite-ffm-runtime:{version}")
}

tasks.test {
    jvmArgs("--enable-native-access=ALL-UNNAMED")
    environment("ETHER_SQLITE_LIB", "/ruta/a/libether_sqlite_ffm_runtime.so")
}
```

### JNI Java 21

```kotlin
java {
    toolchain { languageVersion = JavaLanguageVersion.of(21) }
}

dependencies {
    implementation("mx.rafex.ether:ether-sqlite-jni-runtime:{version}")
}

tasks.test {
    environment("ETHER_SQLITE_JNI_LIB", "/ruta/a/libether_sqlite_jni_runtime.so")
}
```

---

## Línea de comandos (runtime)

### FFM Java 25

```sh
java --enable-native-access=ALL-UNNAMED -jar mi-app.jar
# o con ruta explícita a la librería:
java -Dether.sqlite.lib=/usr/local/lib/libether_sqlite_ffm_runtime.so \
     --enable-native-access=ALL-UNNAMED \
     -jar mi-app.jar
```

### JNI Java 21

```sh
java -jar mi-app.jar
# o con ruta explícita:
java -Dether.sqlite.jni.lib=/usr/local/lib/libether_sqlite_jni_runtime.so \
     -jar mi-app.jar
```

### FFM Java 21 preview

```sh
java --enable-preview --enable-native-access=ALL-UNNAMED -jar mi-app.jar
```

---

## GraalVM Native Image

### FFM Java 25

```xml
<plugin>
  <groupId>org.graalvm.buildtools</groupId>
  <artifactId>native-maven-plugin</artifactId>
  <version>0.10.6</version>
  <configuration>
    <buildArgs>
      <!-- SqliteLibrary carga la .so en static {}: diferir a runtime -->
      <buildArg>--initialize-at-run-time=mx.rafex.ether.sqlite.SqliteLibrary</buildArg>
      <buildArg>--enable-native-access=ALL-UNNAMED</buildArg>
    </buildArgs>
  </configuration>
</plugin>
```

O como binario precompilado del release (`ether-sqlite-ffm-linux-amd64.bin`):

```sh
# Ejecutar directamente
ETHER_SQLITE_LIB=/usr/local/lib/libether_sqlite_ffm_runtime.so ./ether-sqlite-ffm-linux-amd64.bin
```

### JNI Java 21

JNI es plenamente compatible con GraalVM native-image sin configuración especial:

```xml
<plugin>
  <groupId>org.graalvm.buildtools</groupId>
  <artifactId>native-maven-plugin</artifactId>
  <version>0.10.6</version>
  <!-- Sin buildArgs adicionales para JNI -->
</plugin>
```

O como binario precompilado del release (`ether-sqlite-jni-linux-amd64.bin`):

```sh
ETHER_SQLITE_JNI_LIB=/usr/local/lib/libether_sqlite_jni_runtime.so ./ether-sqlite-jni-linux-amd64.bin
```

### FFM Java 21 preview — NO compatible con native-image

El bytecode de Java 21 preview (`minor_version=0xFFFF`) no puede compilarse con GraalVM native-image.
Si necesitas native-image con Java 21, usa el **binding JNI**.

---

## Soporte de plataformas

| OS | Arquitectura | FFM | JNI | Binarios en release |
|---|---|---|---|---|
| Linux | x86\_64 | ✅ | ✅ | ✅ |
| Linux | aarch64 | ✅ | ✅ | ✅ |
| macOS | arm64 (Apple Silicon) | ✅ compilar fuente | ✅ compilar fuente | ❌ |
| macOS | x86\_64 | ✅ compilar fuente | ✅ compilar fuente | ❌ |

### Soporte Raspberry Pi

| Modelo | OS | Soporte |
|---|---|---|
| Raspberry Pi 3B / 4B | 64-bit (aarch64) | ✅ Soportado |
| Raspberry Pi 3B / 4B | 32-bit (armhf) | ❌ No soportado |

### Compilar desde fuente (macOS / otras plataformas)

```sh
git clone https://github.com/rafex/sqlite-native-runtime.git
cd sqlite-native-runtime

# FFM
cargo build -p ether-sqlite-ffm --release --manifest-path sources/rust/Cargo.toml
# Genera: sources/rust/target/release/libether_sqlite_ffm_runtime.dylib

# JNI
cargo build -p ether-sqlite-jni --release --manifest-path sources/rust/Cargo.toml
# Genera: sources/rust/target/release/libether_sqlite_jni_runtime.dylib

# Instalar en macOS
cp sources/rust/target/release/libether_sqlite_ffm_runtime.dylib /usr/local/lib/
cp sources/rust/target/release/libether_sqlite_jni_runtime.dylib /usr/local/lib/
```

---

## Verificar la instalación

```sh
# FFM
nm -D /usr/local/lib/libether_sqlite_ffm_runtime.so | grep snr_open
# Debe mostrar: ... T snr_open

# JNI
nm -D /usr/local/lib/libether_sqlite_jni_runtime.so | grep Java_mx_rafex
# Debe mostrar: ... T Java_mx_rafex_ether_sqlite_jni_SqliteLibraryJni_snrOpen
```
