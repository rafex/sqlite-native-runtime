# scripts/dist/dist.mk — targets de distribución y limpieza.
# Incluido desde el Makefile raíz.

.PHONY: install package clean

install: build
	@$(SCRIPTS_DIR)/dist/install.sh

package: build
	@$(SCRIPTS_DIR)/dist/package.sh

clean:
	@$(SCRIPTS_DIR)/dist/clean.sh
