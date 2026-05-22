CARGO   := $(HOME)/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo
RUSTC   := $(HOME)/.rustup/toolchains/stable-aarch64-apple-darwin/bin/rustc
MVN     := mvn
SNR_LIB := $(PWD)/rust/target/release/libsqlite_native_runtime.dylib

.PHONY: all build-rust build-java build test clean smoke

all: build

build: build-rust build-java

build-rust:
	cd rust && RUSTC=$(RUSTC) $(CARGO) build --release

build-java:
	cd java && $(MVN) compile -q

test: build
	cd java && \
	  SNR_LIB=$(SNR_LIB) \
	  java --enable-native-access=ALL-UNNAMED \
	       -cp target/classes:target/test-classes -ea \
	       mx.rafex.sqlite.SmokeTest

smoke: test

clean:
	cd rust && RUSTC=$(RUSTC) $(CARGO) clean
	cd java && $(MVN) clean -q

package:
	cd java && $(MVN) package -q
