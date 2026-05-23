<!-- RELEASE_TAG: v0.1.3 -->
# Release v0.1.3 — 2026-05-22

### 📝 Documentación
- add installation guide, usage guide, and install.sh script

### 🧪 Tests
- add Dockerfile and suite for validating release artifacts

### ⚙️  CI / Build
- remove workflow_dispatch from verify-release — only triggers after Release
- use debian-slim, run install.sh from release, add verify-release workflow
