GRAALVM_HOME := /Library/Java/JavaVirtualMachines/graalvm-jdk-25.0.2+10.1/Contents/Home
JAVA         := $(GRAALVM_HOME)/bin/java
NATIVE_IMAGE := $(GRAALVM_HOME)/bin/native-image

CARGO    := $(HOME)/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo
RUSTC    := $(HOME)/.rustup/toolchains/stable-aarch64-apple-darwin/bin/rustc

# Maven wrapper — no requiere Maven en el sistema
MVNW    := ./mvnw

SNR_LIB := $(PWD)/sqlite-native-runtime/rust/target/release/libsqlite_native_runtime.dylib

# Librerías Linux (cross-compiladas con cargo-zigbuild)
LINUX_AMD64_LIB := $(PWD)/sqlite-native-runtime/rust/target/x86_64-unknown-linux-gnu/release/libsqlite_native_runtime.so
LINUX_ARM64_LIB := $(PWD)/sqlite-native-runtime/rust/target/aarch64-unknown-linux-gnu/release/libsqlite_native_runtime.so

JAVA_DIR := sqlite-native-runtime/java
RUST_DIR := sqlite-native-runtime/rust

# glibc mínima para máxima compatibilidad en Linux (Ubuntu 18.04+, Debian 10+)
GLIBC_MIN := 2.17

.PHONY: all build-rust build-java build test smoke native \
        cross-linux-amd64 cross-linux-arm64 cross \
        install package test-unit coverage clean

all: build

# ── Build macOS (host) ───────────────────────────────────────────────────────

build: build-rust build-java

build-rust:
	cd $(RUST_DIR) && RUSTC=$(RUSTC) $(CARGO) build --release

# Compila fuentes principales Y de test (necesario para el smoke test manual)
build-java:
	cd $(JAVA_DIR) && JAVA_HOME=$(GRAALVM_HOME) $(MVNW) test-compile -q

test: build
	cd $(JAVA_DIR) && \
	  SNR_LIB=$(SNR_LIB) \
	  $(JAVA) --enable-native-access=ALL-UNNAMED \
	          -cp target/classes:target/test-classes -ea \
	          mx.rafex.sqlite.SmokeTest

smoke: test

# ── Cross-compilación Linux con cargo-zigbuild ───────────────────────────────
# Requisitos (una sola vez):
#   brew install zig
#   cargo install cargo-zigbuild
#   rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
#
# No requiere Docker ni Podman — zig actúa como cross-linker y compilador C.
# El sufijo .$(GLIBC_MIN) fija la glibc mínima para máxima compatibilidad.

cross-linux-amd64:
	cd $(RUST_DIR) && \
	  PATH="$(HOME)/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$$PATH" \
	  $(CARGO) zigbuild --release --target x86_64-unknown-linux-gnu.$(GLIBC_MIN)
	@echo "Librería: $(LINUX_AMD64_LIB)"

cross-linux-arm64:
	cd $(RUST_DIR) && \
	  PATH="$(HOME)/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$$PATH" \
	  $(CARGO) zigbuild --release --target aarch64-unknown-linux-gnu.$(GLIBC_MIN)
	@echo "Librería: $(LINUX_ARM64_LIB)"

# Compila ambos targets Linux
cross: cross-linux-amd64 cross-linux-arm64

# ── Distribución ─────────────────────────────────────────────────────────────

# Genera el JAR e instala en el repositorio Maven local (~/.m2)
install: build
	cd $(JAVA_DIR) && JAVA_HOME=$(GRAALVM_HOME) $(MVNW) install -q

package: build
	cd $(JAVA_DIR) && JAVA_HOME=$(GRAALVM_HOME) $(MVNW) package -q

# Compila el smoke test como ejecutable GraalVM Native Image
# Requiere: make build primero (genera la .dylib y las clases Java)
native: build
	cd $(JAVA_DIR) && \
	  SNR_LIB=$(SNR_LIB) \
	  JAVA_HOME=$(GRAALVM_HOME) \
	  $(MVNW) -Pnative package native:compile -q
	@echo "Binario generado: $(JAVA_DIR)/target/snr-smoke"
	@echo "Ejecutar con: SNR_LIB=$(SNR_LIB) $(JAVA_DIR)/target/snr-smoke"

# ── Tests unitarios y cobertura ──────────────────────────────────────────────

# Ejecuta los tests unitarios JUnit 5 (requiere la .dylib compilada)
test-unit: build-rust
	cd $(JAVA_DIR) && \
	  SNR_LIB=$(SNR_LIB) \
	  JAVA_HOME=$(GRAALVM_HOME) \
	  $(MVNW) test

# Ejecuta tests + verifica cobertura JaCoCo 100% LINE (excluye SqliteLibrary)
# Reporte HTML: sqlite-native-runtime/java/target/site/jacoco/index.html
coverage: build-rust
	cd $(JAVA_DIR) && \
	  SNR_LIB=$(SNR_LIB) \
	  JAVA_HOME=$(GRAALVM_HOME) \
	  $(MVNW) verify
	@echo "Reporte de cobertura: $(JAVA_DIR)/target/site/jacoco/index.html"

# ── Limpieza ─────────────────────────────────────────────────────────────────

clean:
	cd $(RUST_DIR) && RUSTC=$(RUSTC) $(CARGO) clean
	cd $(JAVA_DIR) && JAVA_HOME=$(GRAALVM_HOME) $(MVNW) clean -q
