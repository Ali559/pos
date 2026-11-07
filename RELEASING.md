# Releasing a new version

Installed copies of the app check GitHub for a newer release at startup and every six hours, and offer to install it (a bar at the top; installing restarts the app). Releases are built by GitHub Actions (`.github/workflows/release.yml`) when a version tag is pushed.

## Once: give GitHub the signing key

The updater only installs releases signed with your private key (the public half is in `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`).

In the repository on GitHub: **Settings → Secrets and variables → Actions → New repository secret**, add:

| Name | Value |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | the whole content of the private key file `tauri signer generate` wrote |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | the password you gave it |

Keep the key file and password backed up somewhere safe (a password manager). If they're lost, installed copies can't be updated any more; they'd need a manual reinstall with a new key.

## Each release

1. Make sure `main` is in a good state (tests pass: `bun test tests/` and `cd src-tauri && cargo test`).
2. Set the new version everywhere (`major.minor.patch`, higher than the last):
   ```bash
   bun scripts/bump-version.mjs 0.2.0
   ```
3. Commit, tag and push:
   ```bash
   git commit -am "release: 0.2.0"
   git tag v0.2.0
   git push origin main v0.2.0
   ```
4. Watch the **Actions** tab. When it's done (about 15–25 minutes), the release appears under **Releases** with a Windows installer, Mac builds for both chips, and `latest.json`. Running copies will find it within six hours, or straight away from Settings → About → *Check for updates*.

## Good to know

- The release must be published (not a draft or pre-release) for the updater to see it: `releases/latest` only points at published releases.
- The installers aren't code-signed by Apple or Microsoft yet, so the **first** install shows a warning (macOS: right-click → Open; Windows: *More info* → *Run anyway*). Updates after that install without it. Proper code signing needs an Apple Developer account and a Windows code-signing certificate.
- A shop's data lives in the app's own folder, not in the install, so updating never touches it. Backups (Settings → Backups) are still the safety net.
