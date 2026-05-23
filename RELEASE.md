<!-- RELEASE_TAG: v0.3.0 -->
# Release v0.3.0 — 2026-05-22

### ✨ Nuevas funcionalidades
- Publicación dual de artefactos Java 21 + Java 25

### Artefactos Java 25 (estable, sin flags extra)
- `sqlite-native-runtime-{version}.jar` — thin JAR, requiere Java 25+
- `sqlite-native-runtime-{version}-fat.jar` — fat JAR, requiere Java 25+
- `snr-smoke-linux-amd64` / `snr-smoke-linux-arm64` — nativo x86_64/arm64 compilado con GraalVM 25

### Artefactos Java 21 (preview, requiere `--enable-preview`)
- `sqlite-native-runtime-{version}-java21.jar` — thin JAR, requiere Java 21 + `--enable-preview`
- `sqlite-native-runtime-{version}-java21-fat.jar` — fat JAR, requiere Java 21 + `--enable-preview`
- `snr-smoke-java21-linux-amd64` / `snr-smoke-java21-linux-arm64` — nativo x86_64/arm64, sin flags en runtime

### Librería nativa Rust
- `libsqlite_native_runtime-linux-amd64.so` — glibc 2.17+
- `libsqlite_native_runtime-linux-arm64.so` — glibc 2.17+
