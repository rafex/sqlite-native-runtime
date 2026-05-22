# scripts/container/container.just — recetas de contenedores. Importado desde el Justfile raíz.

container-test-rust:
    scripts/container/container-test.sh rust

container-test-java:
    scripts/container/container-test.sh java

container-test: container-test-rust container-test-java
