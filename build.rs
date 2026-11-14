use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("cargo:rerun-if-env-changed=BUILD_TIMESTAMP");

    let timestamp = std::env::var("BUILD_TIMESTAMP").unwrap_or_else(|_| {
        Command::new("date")
            .args(["-u", "+%Y%m%dT%H%M%SZ"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .to_string()
            })
    });

    println!(
        "cargo:rustc-env=APP_VERSION={}+{}",
        std::env::var("CARGO_PKG_VERSION").expect("Cargo sets CARGO_PKG_VERSION"),
        timestamp
    );
}
