# scripts/test/test.just — recetas de pruebas. Importado desde el Justfile raíz.

test-rust:
    scripts/test/test-rust.sh

test-ffi:
    scripts/test/test-ffi.sh

coverage-rust:
    scripts/test/coverage-rust.sh

test-unit: build-rust
    scripts/test/test-unit.sh

test-integration: build-rust
    scripts/test/test-integration.sh

coverage: build-rust
    scripts/test/coverage-java.sh

# Valida los artefactos del GitHub Release (thin JAR, fat JAR, Native Image pre-compilado)
# en linux/amd64 y linux/arm64 usando Docker buildx.
# Uso: just test-release [version] [platform]
#   just test-release             → último release, ambas archs
#   just test-release v0.1.1      → versión específica, ambas archs
#   just test-release v0.1.1 linux/amd64
test-release version="latest" platform="linux/amd64,linux/arm64":
    scripts/test/test-release.sh {{version}} {{platform}}
