# scripts/dist/dist.just — recetas de distribución y limpieza. Importado desde el Justfile raíz.

install: build
    scripts/dist/install.sh

package: build
    scripts/dist/package.sh

clean:
    scripts/dist/clean.sh
