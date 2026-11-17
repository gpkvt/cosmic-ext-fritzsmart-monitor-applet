//! Package-aware, user-initiated application updates.

use reqwest::Client;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::{fs, process::Command};

const REPOSITORY: &str = "gpkvt/cosmic-ext-fritzsmart-monitor-applet";
const APP_ID: &str = "com.gpkvt.FritzPowerMonitor";
const SOURCE_CHECKOUT: &str = env!("CARGO_MANIFEST_DIR");

/// Install the latest stable release using the mechanism that owns this install.
pub async fn install_update(tag: String) -> Result<String, String> {
    let version = normalize_release_version(&tag)?;
    let tag = format!("v{version}");

    if std::env::var("FLATPAK_ID").as_deref() == Ok(APP_ID) {
        return install_flatpak_update().await;
    }

    if let Ok(appimage) = std::env::var("APPIMAGE") {
        return install_appimage_update(Path::new(&appimage), version, &tag).await;
    }

    if let Some(executable) = debian_package_executable() {
        return install_debian_update(&executable, version, &tag).await;
    }

    if let Some(source) = source_checkout() {
        return install_source_update(&source).await;
    }

    Err("This installation is not managed by a supported updater. Download the package from the project releases page.".into())
}

fn normalize_release_version(tag: &str) -> Result<&str, String> {
    let version = tag.trim().trim_start_matches(['v', 'V']);
    let components: Vec<&str> = version.split('.').collect();
    if components.len() != 3
        || components
            .iter()
            .any(|part| {
            part.is_empty() || !part.chars().all(|character| character.is_ascii_digit())
        })
    {
        return Err("The release tag has an unexpected format".into());
    }
    Ok(version)
}

fn checksum_for_asset<'a>(checksums: &'a str, name: &str) -> Option<&'a str> {
    checksums.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let checksum = fields.next()?;
        let filename = fields.next()?.trim_start_matches('*').trim_start_matches("./");
        (filename == name).then_some(checksum)
    })
}

fn client() -> Result<Client, String> {
    Client::builder()
        .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|error| error.to_string())
}

fn release_asset_url(tag: &str, name: &str) -> String {
    format!("https://github.com/{REPOSITORY}/releases/download/{tag}/{name}")
}

async fn download_verified_asset(tag: &str, name: &str) -> Result<Vec<u8>, String> {
    let client = client()?;
    let checksums = client
        .get(release_asset_url(tag, "SHA256SUMS"))
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .text()
        .await
        .map_err(|error| error.to_string())?;
    let expected = checksum_for_asset(&checksums, name)
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| format!("The release checksum does not list {name}"))?;

    let bytes = client
        .get(release_asset_url(tag, name))
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .bytes()
        .await
        .map_err(|error| error.to_string())?;
    let actual = hex::encode(Sha256::digest(&bytes));
    if actual != expected {
        return Err(format!("SHA-256 verification failed for {name}"));
    }
    Ok(bytes.to_vec())
}

async fn install_appimage_update(
    current: &Path,
    version: &str,
    tag: &str,
) -> Result<String, String> {
    let current = current
        .canonicalize()
        .map_err(|error| format!("Could not locate the running AppImage: {error}"))?;
    if !current.is_file() {
        return Err("The running AppImage path is not a regular file".into());
    }
    let filename = format!("fritz-power-monitor-{version}-x86_64.AppImage");
    let bytes = download_verified_asset(tag, &filename).await?;
    let replacement = current.with_file_name(format!(
        ".fritz-power-monitor-update-{}.AppImage",
        std::process::id()
    ));
    fs::write(&replacement, bytes)
        .await
        .map_err(|error| format!("Could not write the downloaded AppImage: {error}"))?;
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&replacement, std::fs::Permissions::from_mode(0o755))
        .await
        .map_err(|error| format!("Could not mark the AppImage executable: {error}"))?;
    if let Err(error) = fs::rename(&replacement, &current).await {
        let _ = fs::remove_file(&replacement).await;
        return Err(format!("Could not replace the current AppImage: {error}"));
    }
    Ok(format!(
        "Version {version} installed. Restart the applet to use the update."
    ))
}

fn debian_package_executable() -> Option<PathBuf> {
    if std::env::consts::ARCH != "x86_64" {
        return None;
    }
    let executable = std::env::current_exe().ok()?;
    let output = std::process::Command::new("dpkg-query")
        .arg("-S")
        .arg(&executable)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let owner = String::from_utf8_lossy(&output.stdout);
    owner
        .lines()
        .any(|line| line.starts_with("fritz-power-monitor:"))
        .then_some(executable)
}

async fn install_debian_update(
    _executable: &Path,
    version: &str,
    tag: &str,
) -> Result<String, String> {
    let filename = format!("fritz-power-monitor_{version}_amd64.deb");
    let bytes = download_verified_asset(tag, &filename).await?;
    let package = std::env::temp_dir().join(format!(
        "fritz-power-monitor-update-{}-{version}.deb",
        std::process::id()
    ));
    fs::write(&package, bytes)
        .await
        .map_err(|error| format!("Could not save the Debian package: {error}"))?;
    let result = Command::new("pkexec")
        .arg("apt-get")
        .arg("install")
        .arg("-y")
        .arg("--")
        .arg(&package)
        .status()
        .await;
    let _ = fs::remove_file(&package).await;
    let status = result
        .map_err(|error| format!("Could not start the privileged package installer: {error}"))?;
    if !status.success() {
        return Err(format!("The package installer exited with status {status}"));
    }
    Ok(format!(
        "Version {version} installed. Restart the applet to use the update."
    ))
}

fn source_checkout() -> Option<PathBuf> {
    let path = PathBuf::from(SOURCE_CHECKOUT);
    (path.join("Makefile").is_file() && path.join(".git").exists()).then_some(path)
}

async fn install_source_update(source: &Path) -> Result<String, String> {
    let status = Command::new("make")
        .arg("install")
        .arg("UPDATE=1")
        .current_dir(source)
        .status()
        .await
        .map_err(|error| format!("Could not start the source installer: {error}"))?;
    if !status.success() {
        return Err(format!("The source installer exited with status {status}"));
    }
    Ok(
        "The latest source version was built and installed. The COSMIC panel is reloading.".into(),
    )
}

async fn install_flatpak_update() -> Result<String, String> {
    use std::collections::HashMap;
    use zbus::{
        Connection, Proxy,
        zvariant::{OwnedObjectPath, OwnedValue},
    };

    let connection = Connection::session()
        .await
        .map_err(|error| format!("Could not connect to the Flatpak update portal: {error}"))?;
    let portal = Proxy::new(
        &connection,
        "org.freedesktop.portal.Flatpak",
        "/org/freedesktop/portal/Flatpak",
        "org.freedesktop.portal.Flatpak",
    )
    .await
    .map_err(|error| format!("The Flatpak update portal is unavailable: {error}"))?;
    let options: HashMap<String, OwnedValue> = HashMap::new();
    let handle: OwnedObjectPath = portal
        .call("CreateUpdateMonitor", &(options,))
        .await
        .map_err(|error| format!("Could not create a Flatpak update session: {error}"))?;
    let monitor = Proxy::new(
        &connection,
        "org.freedesktop.portal.Flatpak",
        handle.as_str(),
        "org.freedesktop.portal.Flatpak.UpdateMonitor",
    )
    .await
    .map_err(|error| format!("Could not connect to the Flatpak update session: {error}"))?;
    let options: HashMap<String, OwnedValue> = HashMap::new();
    monitor
        .call::<_, _, ()>("Update", &("", options))
        .await
        .map_err(|error| format!("Flatpak could not install this update: {error}"))?;
    Ok(
        "The COSMIC Flatpak updater accepted the request. The update is managed by the system."
            .into(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_tags_must_be_numeric_semver_components() {
        assert_eq!(normalize_release_version("v1.2.3").unwrap(), "1.2.3");
        assert_eq!(normalize_release_version("1.2.3").unwrap(), "1.2.3");
        assert!(normalize_release_version("v1.2.3-rc1").is_err());
        assert!(normalize_release_version("latest").is_err());
    }

    #[test]
    fn checksum_lookup_matches_the_exact_asset_name() {
        let sums = "0123456789abcdef  ./app.deb\nabcdef0123456789 *./app.AppImage\n";
        assert_eq!(checksum_for_asset(sums, "app.deb"), Some("0123456789abcdef"));
        assert_eq!(checksum_for_asset(sums, "app.AppImage"), Some("abcdef0123456789"));
        assert_eq!(checksum_for_asset(sums, "other.deb"), None);
    }

    #[test]
    fn release_asset_urls_are_scoped_to_the_project() {
        assert_eq!(
            release_asset_url("v1.2.3", "SHA256SUMS"),
            "https://github.com/gpkvt/cosmic-ext-fritzsmart-monitor-applet/releases/download/v1.2.3/SHA256SUMS"
        );
    }
}
