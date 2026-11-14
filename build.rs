fn main() {
    println!("cargo:rustc-env=APP_VERSION={}+",
        std::env::var("CARGO_PKG_VERSION").expect("Cargo sets CARGO_PKG_VERSION"));
}
