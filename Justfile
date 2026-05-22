graalvm_home := "/Library/Java/JavaVirtualMachines/graalvm-jdk-25.0.2+10.1/Contents/Home"
java         := graalvm_home + "/bin/java"
native_image := graalvm_home + "/bin/native-image"

cargo := "$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo"
rustc := "$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin/rustc"

# Maven wrapper — no requiere Maven en el sistema
mvnw := "./mvnw"

snr_lib  := justfile_directory() + "/sqlite-native-runtime/rust/target/release/libsqlite_native_runtime.dylib"
java_dir := justfile_directory() + "/sqlite-native-runtime/java"
rust_dir := justfile_directory() + "/sqlite-native-runtime/rust"

linux_amd64_lib := justfile_directory() + "/sqlite-native-runtime/rust/target/x86_64-unknown-linux-gnu/release/libsqlite_native_runtime.so"
linux_arm64_lib := justfile_directory() + "/sqlite-native-runtime/rust/target/aarch64-unknown-linux-gnu/release/libsqlite_native_runtime.so"

# glibc mínima para máxima compatibilidad en Linux (Ubuntu 18.04+, Debian 10+)
glibc_min := "2.17"

default: build

# ── Build macOS (host) ────────────────────────────────────────────────────────

build: build-rust build-java

build-rust:
    cd {{rust_dir}} && RUSTC={{rustc}} {{cargo}} build --release

# Compila fuentes principales Y de test (necesario para el smoke test manual)
build-java:
    cd {{java_dir}} && JAVA_HOME={{graalvm_home}} {{mvnw}} test-compile -q

test: build
    cd {{java_dir}} && \
        SNR_LIB={{snr_lib}} \
        {{java}} --enable-native-access=ALL-UNNAMED \
                 -cp target/classes:target/test-classes -ea \
                 mx.rafex.sqlite.SmokeTest

smoke: test

# ── Cross-compilación Linux con cargo-zigbuild ────────────────────────────────
# Requisitos (una sola vez):
#   brew install zig
#   cargo install cargo-zigbuild
#   rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
#
# No requiere Docker ni Podman — zig actúa como cross-linker y compilador C.

cross-linux-amd64:
    cd {{rust_dir}} && \
        env PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" \
        {{cargo}} zigbuild --release --target x86_64-unknown-linux-gnu.{{glibc_min}}
    @echo "Librería: {{linux_amd64_lib}}"

cross-linux-arm64:
    cd {{rust_dir}} && \
        env PATH="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH" \
        {{cargo}} zigbuild --release --target aarch64-unknown-linux-gnu.{{glibc_min}}
    @echo "Librería: {{linux_arm64_lib}}"

# Compila ambos targets Linux
cross: cross-linux-amd64 cross-linux-arm64

# ── Distribución ──────────────────────────────────────────────────────────────

# Genera el JAR e instala en el repositorio Maven local (~/.m2)
install: build
    cd {{java_dir}} && JAVA_HOME={{graalvm_home}} {{mvnw}} install -q

package: build
    cd {{java_dir}} && JAVA_HOME={{graalvm_home}} {{mvnw}} package -q

# Compila el smoke test como ejecutable GraalVM Native Image
native: build
    cd {{java_dir}} && \
        SNR_LIB={{snr_lib}} \
        JAVA_HOME={{graalvm_home}} \
        {{mvnw}} -Pnative package native:compile -q
    @echo "Binario: {{java_dir}}/target/snr-smoke"

# ── Limpieza ──────────────────────────────────────────────────────────────────

clean:
    cd {{rust_dir}} && RUSTC={{rustc}} {{cargo}} clean
    cd {{java_dir}} && JAVA_HOME={{graalvm_home}} {{mvnw}} clean -q
