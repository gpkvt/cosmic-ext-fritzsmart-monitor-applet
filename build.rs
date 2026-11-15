fn main() {
    let major = std::env::var("CARGO_PKG_VERSION_MAJOR")
        .expect("Cargo sets CARGO_PKG_VERSION_MAJOR");
    let minor = std::env::var("CARGO_PKG_VERSION_MINOR")
        .expect("Cargo sets CARGO_PKG_VERSION_MINOR");
    let patch = std::env::var("CARGO_PKG_VERSION_PATCH")
        .expect("Cargo sets CARGO_PKG_VERSION_PATCH");
    let prerelease = std::env::var("CARGO_PKG_VERSION_PRE")
        .expect("Cargo sets CARGO_PKG_VERSION_PRE");

    println!("cargo:rerun-if-env-changed=APP_RELEASE_VERSION");
    let version = if let Ok(release_version) = std::env::var("APP_RELEASE_VERSION") {
        release_version
    } else {
        let mut version = format!("{major}.{minor}.{patch}");
        if !prerelease.is_empty() {
            version.push('-');
            version.push_str(&prerelease);
        }
        format!("{version}+")
    };

    println!("cargo:rustc-env=APP_VERSION={version}");
}
