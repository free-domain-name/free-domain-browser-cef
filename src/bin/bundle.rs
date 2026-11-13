//! Build and package the app bundle used by CEF on macOS.
#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    use cef::build_util::mac::{build_bundle, BundleInfo};
    use semver::Version;
    use std::path::Path;

    let version = Version::parse(env!("CARGO_PKG_VERSION"))?;
    let info = BundleInfo::new(
        "Free Domain Browser CEF",
        "org.free-domain.browser.cef",
        "Free Domain Browser CEF",
        "English",
        version,
    );
    let app = build_bundle(
        Path::new("target/bundle"),
        "free-domain-browser-cef",
        info,
    )?;

    // `cargo run` should launch the app in this terminal (with logs), not just
    // leave a bundle behind. CEF still needs to be launched from inside the
    // .app, so execute the bundled binary directly.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--build-only") {
        return Ok(());
    }
    let executable = app.join("Contents/MacOS/free-domain-browser-cef");
    let status = std::process::Command::new(&executable)
        .args(args)
        .status()?;
    std::process::exit(status.code().unwrap_or(1));
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("The bundle helper currently targets macOS only.");
}
