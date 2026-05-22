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

# Motor de contenedores: podman (local) o docker (CI).
# Sobreescribir con: make container-test-rust CONTAINER_ENGINE=docker
CONTAINER_ENGINE ?= podman

CONTAINERS_DIR := containers

.PHONY: all build-rust build-java build test smoke native \
        cross-linux-amd64 cross-linux-arm64 cross \
        install package test-unit test-integration coverage \
        test-rust coverage-rust test-ffi \
        container-test-rust container-test-java container-test \
        clean

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

# Ejecuta los tests unitarios Rust (#[cfg(test)]) con cargo test --lib
test-rust:
	cd $(RUST_DIR) && \
	  PATH="$(HOME)/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$$PATH" \
	  RUSTC=$(RUSTC) $(CARGO) test --lib

# Cobertura Rust con cargo-llvm-cov (requiere: cargo install cargo-llvm-cov)
# Reporte HTML: sqlite-native-runtime/rust/target/llvm-cov/html/index.html
coverage-rust:
	cd $(RUST_DIR) && \
	  PATH="$(HOME)/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$$PATH" \
	  RUSTC=$(RUSTC) $(CARGO) llvm-cov --lib --html
	@echo "Reporte de cobertura Rust: $(RUST_DIR)/target/llvm-cov/html/index.html"

# Ejecuta los FFI contract tests (cargo test --test ffi_contract)
# Requiere crate-type = ["cdylib", "staticlib", "rlib"] para linkear el test binary
test-ffi:
	cd $(RUST_DIR) && \
	  PATH="$(HOME)/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$$PATH" \
	  RUSTC=$(RUSTC) $(CARGO) test --test ffi_contract

# Ejecuta los tests unitarios JUnit 5 (requiere la .dylib compilada)
test-unit: build-rust
	cd $(JAVA_DIR) && \
	  SNR_LIB=$(SNR_LIB) \
	  JAVA_HOME=$(GRAALVM_HOME) \
	  $(MVNW) test

# Ejecuta los tests de integración Java (@Tag("integration"))
test-integration: build-rust
	cd $(JAVA_DIR) && \
	  SNR_LIB=$(SNR_LIB) \
	  JAVA_HOME=$(GRAALVM_HOME) \
	  $(MVNW) test -Pintegration

# Ejecuta tests + verifica cobertura JaCoCo 100% LINE (excluye SqliteLibrary)
# Reporte HTML: sqlite-native-runtime/java/target/site/jacoco/index.html
coverage: build-rust
	cd $(JAVA_DIR) && \
	  SNR_LIB=$(SNR_LIB) \
	  JAVA_HOME=$(GRAALVM_HOME) \
	  $(MVNW) verify
	@echo "Reporte de cobertura: $(JAVA_DIR)/target/site/jacoco/index.html"

# ── Tests en contenedor (Podman/Docker) ──────────────────────────────────────
#
# Los mismos Dockerfiles se usan localmente (Podman) y en CI (Docker).
# Sobreescribir el motor con: CONTAINER_ENGINE=docker make container-test-rust
#
# Contexto de build: raíz del repositorio (necesario para COPY de ambos subdirs).
# .dockerignore excluye target/ y .git/ para mantener el contexto ligero.

# TT-1 + TT-2: Rust unit tests + FFI contract tests
container-test-rust:
	$(CONTAINER_ENGINE) build \
	  -f $(CONTAINERS_DIR)/Dockerfile.rust-test \
	  -t snr-rust-test \
	  .
	$(CONTAINER_ENGINE) run --rm snr-rust-test

# TT-3 + TT-3i: Java unit + integration tests (multi-stage: compila .so en el container)
container-test-java:
	$(CONTAINER_ENGINE) build \
	  -f $(CONTAINERS_DIR)/Dockerfile.java-test \
	  -t snr-java-test \
	  .
	$(CONTAINER_ENGINE) run --rm snr-java-test

# Todos los tests en contenedor
container-test: container-test-rust container-test-java

# ── Limpieza ─────────────────────────────────────────────────────────────────

clean:
	cd $(RUST_DIR) && RUSTC=$(RUSTC) $(CARGO) clean
	cd $(JAVA_DIR) && JAVA_HOME=$(GRAALVM_HOME) $(MVNW) clean -q
