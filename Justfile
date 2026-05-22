cargo  := "$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo"
rustc  := "$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin/rustc"
snr_lib := justfile_directory() + "/rust/target/release/libsqlite_native_runtime.dylib"

default: build

build: build-rust build-java

build-rust:
    cd rust && RUSTC={{rustc}} {{cargo}} build --release

build-java:
    cd java && mvn compile -q

test: build
    cd java && \
        SNR_LIB={{snr_lib}} \
        java --enable-native-access=ALL-UNNAMED \
             -cp target/classes:target/test-classes -ea \
             mx.rafex.sqlite.SmokeTest

clean:
    cd rust && RUSTC={{rustc}} {{cargo}} clean
    cd java && mvn clean -q

package: build
    cd java && mvn package -q
