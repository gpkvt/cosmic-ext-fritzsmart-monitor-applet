fn main() {
    let major = std::env::var("CARGO_PKG_VERSION_MAJOR")
        .expect("Cargo sets CARGO_PKG_VERSION_MAJOR");
    let minor = std::env::var("CARGO_PKG_VERSION_MINOR")
        .expect("Cargo sets CARGO_PKG_VERSION_MINOR");
    let patch = std::env::var("CARGO_PKG_VERSION_PATCH")
        .expect("Cargo sets CARGO_PKG_VERSION_PATCH");

    println!("cargo:rerun-if-env-changed=APP_RELEASE_VERSION");
    let version = std::env::var("APP_RELEASE_VERSION")
        .unwrap_or_else(|_| format!("{major}.{minor}.{patch}"));

    println!("cargo:rustc-env=APP_VERSION={version}");
}
