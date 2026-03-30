/// Build script for opt_bt.
///
/// Bakes dependency specs for generated strategy projects at compile time.
///
/// # Dev build (default)
/// Dependencies in generated projects use `path = "..."` pointing to this
/// source tree — exactly as before.
///
/// # Release / `cargo install` build
/// Set these env vars before building / installing:
///   OPT_BT_RELEASE_REPO  e.g. https://github.com/Amol-Gupta/opt_bt
///   OPT_BT_RELEASE_REV   e.g. v0.2.0  or a full commit SHA
///
/// When both are set, generated projects will use
///   `opt_bt = { git = "<repo>", rev = "<rev>" }`
/// so they compile against the same version the user installed, without
/// requiring the source tree to be present.
fn main() {
    let repo = std::env::var("OPT_BT_RELEASE_REPO").unwrap_or_default();
    let rev = std::env::var("OPT_BT_RELEASE_REV").unwrap_or_default();
    let root = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();

    if !repo.is_empty() && !rev.is_empty() {
        // ── Release / cargo-install mode ──────────────────────────────────
        let spec = format!("{{ git = \"{repo}\", rev = \"{rev}\" }}");
        println!("cargo:rustc-env=OPT_BT_DEP_ROOT=opt_bt = {spec}");
        println!("cargo:rustc-env=OPT_BT_DEP_SDK=bt_strategy_sdk = {spec}");
        println!("cargo:rustc-env=OPT_BT_DEP_MACROS=bt_strategy_macros = {spec}");
        // Engine binary comes from PATH (already installed); no source path.
        println!("cargo:rustc-env=OPT_BT_ENGINE_PATH=");
    } else {
        // ── Dev / source build mode ───────────────────────────────────────
        println!("cargo:rustc-env=OPT_BT_DEP_ROOT=opt_bt = {{ path = \"{root}\" }}");
        println!(
            "cargo:rustc-env=OPT_BT_DEP_SDK=bt_strategy_sdk = {{ path = \"{root}/crates/bt_strategy_sdk\" }}"
        );
        println!(
            "cargo:rustc-env=OPT_BT_DEP_MACROS=bt_strategy_macros = {{ path = \"{root}/crates/bt_strategy_macros\" }}"
        );
        println!("cargo:rustc-env=OPT_BT_ENGINE_PATH={root}");
    }

    // Re-run this script if either release env var changes.
    println!("cargo:rerun-if-env-changed=OPT_BT_RELEASE_REPO");
    println!("cargo:rerun-if-env-changed=OPT_BT_RELEASE_REV");
}
