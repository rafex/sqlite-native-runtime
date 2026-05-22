cargo  := "$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo"
rustc  := "$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin/rustc"
snr_lib := justfile_directory() + "/sqlite-native-runtime/rust/target/release/libsqlite_native_runtime.dylib"

default: build

build: build-rust build-java

build-rust:
    cd sqlite-native-runtime/rust && RUSTC={{rustc}} {{cargo}} build --release

build-java:
    cd sqlite-native-runtime/java && mvn compile -q

test: build
    cd sqlite-native-runtime/java && \
        SNR_LIB={{snr_lib}} \
        java --enable-native-access=ALL-UNNAMED \
             -cp target/classes:target/test-classes -ea \
             mx.rafex.sqlite.SmokeTest

clean:
    cd sqlite-native-runtime/rust && RUSTC={{rustc}} {{cargo}} clean
    cd sqlite-native-runtime/java && mvn clean -q

package: build
    cd sqlite-native-runtime/java && mvn package -q
