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
    build_bundle(
        Path::new("target/bundle"),
        "free-domain-browser-cef",
        info,
    )?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("The bundle helper currently targets macOS only.");
}
