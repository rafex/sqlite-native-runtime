CARGO   := $(HOME)/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo
RUSTC   := $(HOME)/.rustup/toolchains/stable-aarch64-apple-darwin/bin/rustc
MVN     := mvn
SNR_LIB := $(PWD)/sqlite-native-runtime/rust/target/release/libsqlite_native_runtime.dylib

.PHONY: all build-rust build-java build test clean smoke

all: build

build: build-rust build-java

build-rust:
	cd sqlite-native-runtime/rust && RUSTC=$(RUSTC) $(CARGO) build --release

build-java:
	cd sqlite-native-runtime/java && $(MVN) compile -q

test: build
	cd sqlite-native-runtime/java && \
	  SNR_LIB=$(SNR_LIB) \
	  java --enable-native-access=ALL-UNNAMED \
	       -cp target/classes:target/test-classes -ea \
	       mx.rafex.sqlite.SmokeTest

smoke: test

clean:
	cd sqlite-native-runtime/rust && RUSTC=$(RUSTC) $(CARGO) clean
	cd sqlite-native-runtime/java && $(MVN) clean -q

package:
	cd sqlite-native-runtime/java && $(MVN) package -q
