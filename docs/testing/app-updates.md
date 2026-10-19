# Application updates

Version 0.10.0 checks the public Crown Mod GitHub release feed once at startup.
It also provides **Check for updates**. Only newer stable semantic versions are
offered; drafts, prereleases, unknown tags, equal versions and older versions are
ignored. Notes for the newer releases are shown newest first. Being offline at
startup is quiet; a manual check reports a connection failure and allows retry.

The offer waits until disc analysis, patching and other notices finish. **Later**
or dismissal takes no update action. Accepting the offer:

- **Windows MSI installation:** checks registered products with the existing
  upgrade GUID and requires the executable's directory to match the registered
  InstallLocation. A portable copy elsewhere therefore uses the browser even if
  another MSI copy is installed. The matching x86_64 MSI must have a GitHub SHA-256
  digest. The accepted version's installer is downloaded over HTTPS, its size and
  SHA-256 verified, then Windows Installer opens with the normal UI and automatic
  reboot disabled. The app closes after successful installer launch. Its fixed
  upgrade GUID and installation identifier remain unchanged.
- **Portable Windows or Linux:** opens the selected GitHub release page through
  the default browser. No executable is downloaded or run by the app.
- Missing/unverifiable MSI assets or unrecognised installation registration fall
  back to the release page.

The native side caches the checked offer and accepts only its version; the
webview cannot pass an arbitrary installer URL. Patching and update acceptance
share an operation permit. Failed downloads are discarded; no installer runs
until verification passes. A successfully launched MSI remains in the app's cache
so Windows Installer can finish reading it after the app closes.

## Release publishing

`release.yml` calls the Windows and Linux build workflows, waits for both, checks
all five download files, uploads them to a draft release, then publishes it. This
keeps incomplete releases out of the update feed. Both release notes and the
in-app changelog come from `CHANGELOG.md`, through `tools/release-notes.mjs` and
GitHub's release body. The script requires the tag, package.json, both Rust crates
and Tauri config to agree on the app version. No tag or release is created by
editing these files locally.

Older app versions need a manual download of 0.10.0 to acquire this feature.
Existing patched game images are unaffected by an application update.

## Validation

- Frontend tests exercise update notes, explicit acceptance, Later, deferred
  offers while busy, offline startup/manual retry, MSI handoff, errors and
  recovery, and disc-error/update-dialog ordering.
- Native tests cover semantic versions, stable-only filtering, cumulative notes,
  MSI/portable routing, trusted asset metadata, installation-directory matching,
  upgrade identity, exclusive operations and truncated/tampered downloads.
- The live GitHub check passed. A real public MSI was downloaded and its GitHub
  size/SHA-256 verified in a temporary directory; it was never executed.
- The running Linux application passed startup/manual checking against the live
  feed, the up-to-date notice and refusal of an unselected update version. A
  rejected update releases the operation permit. The requested footer is absent.
- Node tests cover release-note extraction, version mismatch and empty notes.
  Workflow YAML parses successfully. Native tests are included in both Windows
  and Linux shell CI jobs.
- Actual Windows Installer execution/upgrade is not tested on the Linux host.
  Before publishing, smoke-test an MSI-installed copy on Windows, plus a portable
  executable outside its install directory. Verify the former uses the MSI and
  the latter opens GitHub. Decline before accepting to confirm no action occurs.

Commands:

```sh
npm test
npm run lint
npm run build
cargo test --manifest-path crates/dw4vhp-tauri/Cargo.toml
cargo test --manifest-path crates/dw4vhp-tauri/Cargo.toml live_public_release_check -- --ignored
cargo test --manifest-path crates/dw4vhp-tauri/Cargo.toml live_installer_download -- --ignored
node tools/release-notes.mjs --tag v0.10.0 --output release-notes.md
```

The live installer test verifies a download only; it does not install anything.
API behavior follows [GitHub's release API](https://docs.github.com/en/rest/releases/releases).
MSI detection uses [MsiEnumRelatedProductsW](https://learn.microsoft.com/en-us/windows/win32/api/msi/nf-msi-msienumrelatedproductsw)
and [MsiGetProductInfoW](https://learn.microsoft.com/en-us/windows/win32/api/msi/nf-msi-msigetproductinfow).
The installer is launched using [Windows Installer's standard options](https://learn.microsoft.com/en-us/windows/win32/msi/standard-installer-command-line-options).


## Linux older-version regression (2026-10-19)

The Linux production selector and frontend tests cover installed v0.10.0 with a
v0.11.0 stable release: the offer contains both versions, release notes and the
GitHub release page. An installed v0.11.0 receives no offer for that same release.
The production updater implementation is unchanged from the v0.10.0 tag.

A new opt-in Linux network test uses the real production `fetch` function and
Linux's `is_msi_install()` result, without opening a browser or installing anything:

```sh
cargo test --manifest-path crates/dw4vhp-tauri/Cargo.toml \
  live_linux_older_build_detects_update -- --ignored --nocapture
```

The 2026-10-19 live recheck passed against GitHub: an installed v0.10.0
received an offer for v0.11.0, including release notes and the browser update
method used on Linux. No browser was opened and no installer was executed.
