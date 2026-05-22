# scripts/commons/commons.just — variables compartidas para todos los sub-justfiles.
# Importado automáticamente desde el Justfile raíz con: import 'scripts/commons/commons.just'

# ── Toolchains ────────────────────────────────────────────────────────────────
# Las variables exportadas con `export` están disponibles como variables de entorno
# en cada receta, ignorando lo que tenga el shell del usuario.
# Para sobreescribir desde la línea de comandos:
#   just build-rust GRAALVM_HOME=/otra/ruta
export GRAALVM_HOME    := "/Library/Java/JavaVirtualMachines/graalvm-jdk-25.0.2+10.1/Contents/Home"
export CARGO_TOOLCHAIN := "stable-aarch64-apple-darwin"

# ── Variables sobreescribibles desde el entorno o la CLI ─────────────────────
export CONTAINER_ENGINE := env_var_or_default("CONTAINER_ENGINE", "podman")
export GLIBC_MIN        := env_var_or_default("GLIBC_MIN", "2.17")
