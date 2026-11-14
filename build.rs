fn main() {
    let major = std::env::var("CARGO_PKG_VERSION_MAJOR")
        .expect("Cargo sets CARGO_PKG_VERSION_MAJOR");
    let minor = std::env::var("CARGO_PKG_VERSION_MINOR")
        .expect("Cargo sets CARGO_PKG_VERSION_MINOR");
    let patch = std::env::var("CARGO_PKG_VERSION_PATCH")
        .expect("Cargo sets CARGO_PKG_VERSION_PATCH");
    let prerelease = std::env::var("CARGO_PKG_VERSION_PRE")
        .expect("Cargo sets CARGO_PKG_VERSION_PRE");

    let mut version = format!("{major}.{minor}.{patch}");
    if !prerelease.is_empty() {
        version.push('-');
        version.push_str(&prerelease);
    }

    println!("cargo:rustc-env=APP_VERSION={version}+");
}
