graalvm_home := "/Library/Java/JavaVirtualMachines/graalvm-jdk-25.0.2+10.1/Contents/Home"
java         := graalvm_home + "/bin/java"
native_image := graalvm_home + "/bin/native-image"

cargo  := "$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin/cargo"
rustc  := "$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin/rustc"

snr_lib  := justfile_directory() + "/sqlite-native-runtime/rust/target/release/libsqlite_native_runtime.dylib"
java_dir := justfile_directory() + "/sqlite-native-runtime/java"
rust_dir := justfile_directory() + "/sqlite-native-runtime/rust"

default: build

build: build-rust build-java

build-rust:
    cd {{rust_dir}} && RUSTC={{rustc}} {{cargo}} build --release

# Compila fuentes principales Y de test (necesario para el smoke test manual)
build-java:
    cd {{java_dir}} && JAVA_HOME={{graalvm_home}} mvn test-compile -q

test: build
    cd {{java_dir}} && \
        SNR_LIB={{snr_lib}} \
        {{java}} --enable-native-access=ALL-UNNAMED \
                 -cp target/classes:target/test-classes -ea \
                 mx.rafex.sqlite.SmokeTest

smoke: test

# Compila el smoke test como ejecutable GraalVM Native Image
native: build
    cd {{java_dir}} && \
        SNR_LIB={{snr_lib}} \
        JAVA_HOME={{graalvm_home}} \
        mvn -Pnative package native:compile -q
    echo "Binario: {{java_dir}}/target/snr-smoke"

clean:
    cd {{rust_dir}} && RUSTC={{rustc}} {{cargo}} clean
    cd {{java_dir}} && JAVA_HOME={{graalvm_home}} mvn clean -q

package: build
    cd {{java_dir}} && JAVA_HOME={{graalvm_home}} mvn package -q
