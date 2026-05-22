# scripts/build/build.just — recetas de compilación. Importado desde el Justfile raíz.

build-rust:
    scripts/build/build-rust.sh

build-java:
    scripts/build/build-java.sh

smoke:
    scripts/build/smoke.sh

native: build
    scripts/build/native.sh

cross-linux-amd64:
    scripts/build/cross.sh amd64

cross-linux-arm64:
    scripts/build/cross.sh arm64

cross: cross-linux-amd64 cross-linux-arm64
