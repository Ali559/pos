# TODO

## Auto-update

Built (2026-11-07): `tauri-plugin-updater` with the public key and the GitHub URL in `src-tauri/tauri.conf.json`, a bar in the app when an update is ready, Settings → About → *Check for updates*, and the release workflow `.github/workflows/release.yml`. How to release: [RELEASING.md](RELEASING.md).

- [x] **A signing key.** Generated; the public key is in `tauri.conf.json`.
- [x] **Where releases are published.** GitHub releases on `Ali559/pos` (public).
- [ ] **Add the two secrets on GitHub** (`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`): see RELEASING.md. Only you can do this; the private key must never go in the repo.
- [ ] **First release** (`v0.1.0` or `v0.2.0`) to check the workflow builds on Windows and macOS, then a second one to see an installed copy update itself.
- [ ] Later: Apple and Windows code signing, so the first install doesn't show a security warning.
