# scripts/test/test.mk — targets de tests y cobertura.
# Incluido desde el Makefile raíz.

.PHONY: test-rust test-ffi coverage-rust \
        test-unit test-integration coverage

# ── Rust ──────────────────────────────────────────────────────────────────────

test-rust:
	@$(SCRIPTS_DIR)/test/test-rust.sh

test-ffi:
	@$(SCRIPTS_DIR)/test/test-ffi.sh

coverage-rust:
	@$(SCRIPTS_DIR)/test/coverage-rust.sh

# ── Java ──────────────────────────────────────────────────────────────────────

test-unit: build-rust
	@$(SCRIPTS_DIR)/test/test-unit.sh

test-integration: build-rust
	@$(SCRIPTS_DIR)/test/test-integration.sh

coverage: build-rust
	@$(SCRIPTS_DIR)/test/coverage-java.sh
