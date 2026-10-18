//! User-approved GitHub release updates. Never executes a URL from the webview.
use reqwest::{blocking::Client, Url};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, io::Read, sync::Mutex, time::Duration};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

#[cfg(windows)]
mod windows;
const API: &str = "https://api.github.com/repos/crashbashash/DW4_Crown_Mod/releases?per_page=100";
const RELEASES: &str = "https://github.com/crashbashash/DW4_Crown_Mod/releases";
const MAX_RESPONSE: u64 = 2 * 1024 * 1024;
const MAX_INSTALLER: u64 = 500 * 1024 * 1024;

#[derive(Default)]
pub struct UpdateState {
    pending: Mutex<Option<Prepared>>,
}
#[derive(Clone, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    body: Option<String>,
    #[serde(default)]
    assets: Vec<Asset>,
}
#[derive(Clone, Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseNotes {
    version: String,
    notes: String,
}
#[derive(Clone, Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOffer {
    version: String,
    current_version: String,
    releases: Vec<ReleaseNotes>,
    method: &'static str,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    current_version: String,
    update: Option<UpdateOffer>,
}
#[derive(Clone)]
struct Prepared {
    offer: UpdateOffer,
    page: String,
    installer: Option<Asset>,
}
fn client(timeout: Duration) -> Result<Client, String> {
    Client::builder()
        .user_agent(concat!("DW4-Crown-Mod/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(5))
        .timeout(timeout)
        .https_only(true)
        .build()
        .map_err(|_| "Unable to connect to GitHub. Try again later.".into())
}
fn version(tag: &str) -> Option<Version> {
    Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok()
}
fn installer_asset(asset: &Asset, tag: &str) -> bool {
    let Some(version) = version(tag) else {
        return false;
    };
    let expected = ["dw4-crown-mod", "dw4-veryhard-plus"]
        .map(|prefix| format!("{prefix}-{version}-windows-x86_64.msi"));
    let Ok(url) = Url::parse(&asset.browser_download_url) else {
        return false;
    };
    let trusted_path = ["DW4_Crown_Mod", "DW4_VeryHard_Plus"].iter().any(|repo| {
        url.path()
            == format!(
                "/crashbashash/{repo}/releases/download/{tag}/{}",
                asset.name
            )
    });
    asset.size > 0
        && asset.size <= MAX_INSTALLER
        && expected.contains(&asset.name)
        && url.scheme() == "https"
        && url.host_str() == Some("github.com")
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && trusted_path
        && asset.digest.as_deref().is_some_and(|digest| {
            digest
                .strip_prefix("sha256:")
                .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|c| c.is_ascii_hexdigit()))
        })
}
fn prepare(releases: Vec<Release>, current: &Version, msi: bool) -> Option<Prepared> {
    let mut newer: Vec<_> = releases
        .into_iter()
        .filter_map(|r| {
            let v = version(&r.tag_name)?;
            (!r.draft && !r.prerelease && v.pre.is_empty() && v.cmp_precedence(current).is_gt())
                .then_some((v, r))
        })
        .collect();
    newer.sort_by(|a, b| b.0.cmp_precedence(&a.0));
    let (latest, release) = newer.first()?;
    let installer = msi
        .then(|| {
            release
                .assets
                .iter()
                .find(|a| installer_asset(a, &release.tag_name))
                .cloned()
        })
        .flatten();
    let mut page = Url::parse(RELEASES).unwrap();
    page.path_segments_mut()
        .unwrap()
        .extend(["tag", &release.tag_name]);
    let mut seen = BTreeSet::new();
    let notes = newer
        .iter()
        .filter(|(v, _)| seen.insert(v.to_string()))
        .map(|(v, r)| ReleaseNotes {
            version: v.to_string(),
            notes: r
                .body
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("See the GitHub release page for details.")
                .into(),
        })
        .collect();
    Some(Prepared {
        offer: UpdateOffer {
            version: latest.to_string(),
            current_version: current.to_string(),
            releases: notes,
            method: if installer.is_some() {
                "msi"
            } else {
                "browser"
            },
        },
        page: page.into(),
        installer,
    })
}
fn fetch(current: &Version, msi: bool) -> Result<Option<Prepared>, String> {
    let response = client(Duration::from_secs(15))?
        .get(API)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|_| {
            "Couldn't check GitHub for updates. Check your connection and try again later."
                .to_string()
        })?;
    let mut bytes = Vec::new();
    response
        .take(MAX_RESPONSE + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Couldn't read the update information. Try again later.")?;
    if bytes.len() as u64 > MAX_RESPONSE {
        return Err("GitHub's update response was too large. Try again later.".into());
    }
    let releases = serde_json::from_slice(&bytes)
        .map_err(|_| "GitHub returned unreadable update information. Try again later.")?;
    Ok(prepare(releases, current, msi))
}

#[tauri::command]
pub async fn check_updates(app: AppHandle) -> Result<UpdateCheck, String> {
    let current = app.package_info().version.clone();
    let msi = is_msi_install();
    let prepared = tauri::async_runtime::spawn_blocking(move || fetch(&current, msi))
        .await
        .map_err(|_| "Couldn't check for updates. Try again later.")??;
    let update = prepared.as_ref().map(|p| p.offer.clone());
    *app.state::<UpdateState>()
        .pending
        .lock()
        .map_err(|_| "The update check is unavailable. Restart the app.")? = prepared;
    Ok(UpdateCheck {
        current_version: app.package_info().version.to_string(),
        update,
    })
}
fn is_msi_install() -> bool {
    #[cfg(windows)]
    {
        windows::is_msi_install()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[tauri::command]
pub async fn accept_update(version: String, app: AppHandle) -> Result<&'static str, String> {
    let permit = app.state::<crate::state::AppState>().begin_operation()?;
    let prepared = app
        .state::<UpdateState>()
        .pending
        .lock()
        .map_err(|_| "The update is unavailable. Check again.")?
        .clone()
        .filter(|p| p.offer.version == version)
        .ok_or("This update is no longer selected. Check for updates again.")?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        if let Some(asset) = prepared.installer {
            #[cfg(windows)]
            {
                if !windows::is_msi_install() {
                    return Err(
                        "This copy is portable. Check for updates again to open the release page."
                            .into(),
                    );
                }
                let folder = app
                    .path()
                    .app_cache_dir()
                    .map_err(|e| e.to_string())?
                    .join("updates");
                std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
                let dir = tempfile::Builder::new()
                    .prefix("crown-update-")
                    .tempdir_in(folder)
                    .map_err(|e| e.to_string())?;
                let file = dir.path().join("update.msi");
                download(&asset, &file)?;
                windows::launch_installer(&file)?;
                let _retained = dir.keep(); // Windows Installer still needs the file after we exit.
                app.exit(0);
                return Ok("installer");
            }
            #[cfg(not(windows))]
            {
                let _ = asset;
            }
        }
        app.opener()
            .open_url(&prepared.page, None::<&str>)
            .map_err(|_| {
                "Couldn't open your browser. Visit the Crown Mod GitHub releases page to update."
                    .to_string()
            })?;
        Ok("browser")
    })
    .await
    .map_err(|_| "The update could not be started. Try again.")?
}

#[cfg(any(windows, test))]
fn copy_installer(
    mut input: impl Read,
    mut output: impl std::io::Write,
    size: u64,
    digest: &str,
) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    let mut written = 0u64;
    let mut buffer = [0; 65536];
    loop {
        let len = input
            .read(&mut buffer)
            .map_err(|_| "The update download was interrupted. Try again.")?;
        if len == 0 {
            break;
        }
        written += len as u64;
        if written > size || written > MAX_INSTALLER {
            return Err(
                "The update download had an unexpected size. Check for updates again.".into(),
            );
        }
        hash.update(&buffer[..len]);
        output.write_all(&buffer[..len]).map_err(|_| {
            "The installer could not be saved. Check your free space and try again."
        })?;
    }
    if written != size || format!("sha256:{:x}", hash.finalize()) != digest.to_ascii_lowercase() {
        return Err("The installer failed verification. Nothing was installed. Check for updates and try again.".into());
    }
    Ok(())
}
#[cfg(any(windows, test))]
fn download(asset: &Asset, path: &std::path::Path) -> Result<(), String> {
    let response = client(Duration::from_secs(300))?
        .get(&asset.browser_download_url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|_| "Couldn't download the installer. Check your connection and try again.")?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| "Couldn't create the installer file. Check your free space and try again.")?;
    copy_installer(
        response,
        &mut file,
        asset.size,
        asset
            .digest
            .as_deref()
            .ok_or("The installer has no verification checksum.")?,
    )?;
    file.sync_all()
        .map_err(|_| "Couldn't finish saving the installer. Try again.".into())
}

#[cfg(any(windows, test))]
fn same_install_location(executable: &std::path::Path, registered: &std::path::Path) -> bool {
    let (Ok(executable), Ok(registered)) = (executable.canonicalize(), registered.canonicalize())
    else {
        return false;
    };
    executable.parent() == Some(registered.as_path())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn releases() -> Vec<Release> {
        serde_json::from_value(serde_json::json!([
            {"tag_name":"v0.10.0","body":"Newest changes","assets":[{"name":"dw4-crown-mod-0.10.0-windows-x86_64.msi","browser_download_url":"https://github.com/crashbashash/DW4_Crown_Mod/releases/download/v0.10.0/dw4-crown-mod-0.10.0-windows-x86_64.msi","size":3,"digest":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}]},
            {"tag_name":"v0.9.1","body":"Earlier fixes"},
            {"tag_name":"v0.9.0","body":"Already installed"},
            {"tag_name":"v1.0.0-beta.1","body":"Beta"},
            {"tag_name":"v2.0.0","prerelease":true},
            {"tag_name":"v3.0.0","draft":true},
            {"tag_name":"not-a-version"}
        ])).unwrap()
    }
    #[test]
    fn stable_semver_and_notes_cover_every_newer_release() {
        let p = prepare(releases(), &Version::parse("0.9.0").unwrap(), false).unwrap();
        assert_eq!(p.offer.version, "0.10.0");
        assert_eq!(p.offer.method, "browser");
        assert_eq!(
            p.offer
                .releases
                .iter()
                .map(|r| r.version.as_str())
                .collect::<Vec<_>>(),
            vec!["0.10.0", "0.9.1"]
        );
        assert!(prepare(releases(), &Version::parse("0.10.0").unwrap(), false).is_none());
        assert!(prepare(releases(), &Version::parse("0.10.0+local").unwrap(), false).is_none());
        assert!(prepare(vec![], &Version::parse("0.1.0").unwrap(), false).is_none());
    }
    #[test]
    fn only_registered_msi_installs_with_verified_assets_use_the_installer() {
        let current = Version::parse("0.9.0").unwrap();
        assert_eq!(
            prepare(releases(), &current, true).unwrap().offer.method,
            "msi"
        );
        for field in ["digest", "url", "size", "name"] {
            let mut r = releases();
            let a = &mut r[0].assets[0];
            match field {
                "digest" => a.digest = None,
                "url" => a.browser_download_url = "https://example.com/installer.msi".into(),
                "size" => a.size = MAX_INSTALLER + 1,
                _ => a.name = "other.msi".into(),
            }
            assert_eq!(prepare(r, &current, true).unwrap().offer.method, "browser");
        }
    }
    #[test]
    fn truncated_tampered_and_oversized_installers_are_rejected() {
        use sha2::{Digest, Sha256};
        let digest = format!("sha256:{:x}", Sha256::digest(b"abc"));
        assert!(copy_installer(b"abc".as_slice(), Vec::new(), 3, &digest).is_ok());
        for bytes in [b"ab".as_slice(), b"abcd", b"xyz"] {
            assert!(copy_installer(bytes, Vec::new(), 3, &digest).is_err());
        }
    }
    #[test]
    fn portable_copy_is_not_an_msi_install_even_when_another_copy_is_registered() {
        let root = tempfile::tempdir().unwrap();
        let installed = root.path().join("installed");
        let portable = root.path().join("installed-copy");
        for dir in [&installed, &portable] {
            std::fs::create_dir(dir).unwrap();
            std::fs::write(dir.join("app.exe"), b"fixture").unwrap();
        }
        assert!(same_install_location(
            &installed.join("app.exe"),
            &installed
        ));
        assert!(!same_install_location(
            &portable.join("app.exe"),
            &installed
        ));
        assert!(!same_install_location(
            &installed.join("app.exe"),
            &root.path().join("missing")
        ));
    }
    #[test]
    fn installed_upgrade_identity_stays_pinned() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(
            config["bundle"]["windows"]["wix"]["upgradeCode"],
            "bd9d3915-b839-48c7-ae79-afbf60a6cd78"
        );
    }
    #[test]
    #[ignore = "downloads a public MSI for verification only; never executes it"]
    fn live_installer_download_is_verified_without_executing_it() {
        let prepared = fetch(&Version::parse("0.1.0").unwrap(), true)
            .unwrap()
            .unwrap();
        let asset = prepared.installer.unwrap();
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("download.msi");
        download(&asset, &path).unwrap();
        assert_eq!(std::fs::metadata(path).unwrap().len(), asset.size);
    }
    #[test]
    #[ignore = "live GitHub check; explicitly run when network is available"]
    fn live_public_release_check() {
        let p = fetch(&Version::parse("0.1.0").unwrap(), true)
            .unwrap()
            .unwrap();
        assert!(Version::parse(&p.offer.version).unwrap() > Version::parse("0.1.0").unwrap());
        assert!(!p.offer.releases.is_empty());
        assert_eq!(p.offer.method, "msi");
    }
}
