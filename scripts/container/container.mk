# scripts/container/container.mk — targets de tests en contenedor (Podman/Docker).
# Incluido desde el Makefile raíz.
#
# Motor por defecto: podman. Sobreescribir con: CONTAINER_ENGINE=docker make ...

.PHONY: container-test-rust container-test-java container-test

container-test-rust:
	@$(SCRIPTS_DIR)/container/container-test.sh rust

container-test-java:
	@$(SCRIPTS_DIR)/container/container-test.sh java

container-test: container-test-rust container-test-java
