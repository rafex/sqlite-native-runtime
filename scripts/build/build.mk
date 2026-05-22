# scripts/build/build.mk — targets de compilación.
# Incluido desde el Makefile raíz.

.PHONY: build-rust build-java build smoke native \
        cross-linux-amd64 cross-linux-arm64 cross

build-rust:
	@$(SCRIPTS_DIR)/build/build-rust.sh

build-java:
	@$(SCRIPTS_DIR)/build/build-java.sh

smoke:
	@$(SCRIPTS_DIR)/build/smoke.sh

native:
	@$(SCRIPTS_DIR)/build/native.sh

cross-linux-amd64:
	@$(SCRIPTS_DIR)/build/cross.sh amd64

cross-linux-arm64:
	@$(SCRIPTS_DIR)/build/cross.sh arm64

cross: cross-linux-amd64 cross-linux-arm64
