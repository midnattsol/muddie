//! What linking Muddie needs beyond what Cargo knows.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        link_clang_runtime();
    }
}

/// The speech of macOS, in prism, asks which version of the system it runs
/// on as Objective-C does (`@available`), which needs
/// `__isPlatformVersionAtLeast` from clang's runtime: Rust does not link it,
/// so it is linked here, from where the clang of the system keeps it.
fn link_clang_runtime() {
    let resources = std::process::Command::new("xcrun")
        .args(["clang", "-print-resource-dir"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned());
    let Some(resources) = resources else {
        println!("cargo:warning=clang's runtime was not found: linking may fail");
        return;
    };
    let darwin = std::path::Path::new(&resources).join("lib/darwin");
    if !darwin.join("libclang_rt.osx.a").is_file() {
        println!("cargo:warning=no libclang_rt.osx.a in {}", darwin.display());
        return;
    }
    println!("cargo:rustc-link-search=native={}", darwin.display());
    println!("cargo:rustc-link-lib=static=clang_rt.osx");
}
