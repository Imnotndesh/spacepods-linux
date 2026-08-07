fn main() {
    // Use GIT_TAG env var if set (CI), otherwise fall back to Cargo.toml version
    let version = std::env::var("GIT_TAG")
        .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());
    // Strip leading 'v' if present
    let version = version.strip_prefix('v').unwrap_or(&version);
    println!("cargo:rustc-env=SPACEPODS_VERSION={}", version);
    // Re-run if GIT_TAG changes
    println!("cargo:rerun-if-env-changed=GIT_TAG");
}
