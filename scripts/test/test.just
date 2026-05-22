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
