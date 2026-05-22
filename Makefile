GRAALVM_HOME := /Library/Java/JavaVirtualMachines/graalvm-jdk-25.0.2+10.1/Contents/Home
JAVA         := $(GRAALVM_HOME)/bin/java
NATIVE_IMAGE := $(GRAALVM_HOME)/bin/native-image

CARGO   := $(HOME)/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo
RUSTC   := $(HOME)/.rustup/toolchains/stable-aarch64-apple-darwin/bin/rustc
MVN     := mvn

SNR_LIB := $(PWD)/sqlite-native-runtime/rust/target/release/libsqlite_native_runtime.dylib

JAVA_DIR := sqlite-native-runtime/java
RUST_DIR := sqlite-native-runtime/rust

.PHONY: all build-rust build-java build test native clean smoke package

all: build

build: build-rust build-java

build-rust:
	cd $(RUST_DIR) && RUSTC=$(RUSTC) $(CARGO) build --release

# Compila fuentes principales Y de test (necesario para el smoke test manual)
build-java:
	JAVA_HOME=$(GRAALVM_HOME) cd $(JAVA_DIR) && $(MVN) test-compile -q

test: build
	cd $(JAVA_DIR) && \
	  SNR_LIB=$(SNR_LIB) \
	  $(JAVA) --enable-native-access=ALL-UNNAMED \
	          -cp target/classes:target/test-classes -ea \
	          mx.rafex.sqlite.SmokeTest

smoke: test

# Compila el smoke test como ejecutable GraalVM Native Image
# Requiere: make build primero (genera la .dylib y las clases Java)
native: build
	SNR_LIB=$(SNR_LIB) \
	JAVA_HOME=$(GRAALVM_HOME) \
	cd $(JAVA_DIR) && $(MVN) -Pnative package native:compile -q
	@echo "Binario generado: $(JAVA_DIR)/target/snr-smoke"
	@echo "Ejecutar con: SNR_LIB=$(SNR_LIB) $(JAVA_DIR)/target/snr-smoke"

clean:
	cd $(RUST_DIR) && RUSTC=$(RUSTC) $(CARGO) clean
	JAVA_HOME=$(GRAALVM_HOME) cd $(JAVA_DIR) && $(MVN) clean -q

package:
	JAVA_HOME=$(GRAALVM_HOME) cd $(JAVA_DIR) && $(MVN) package -q
