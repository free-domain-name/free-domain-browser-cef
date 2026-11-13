//! Build and package the app bundle used by CEF on macOS.
#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    use cef::build_util::mac::{bundle, BundleInfo};
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
    // Build the app and helper in a separate target directory. Calling cargo
    // again in the normal target directory would deadlock the `cargo run` that
    // launched this binary.
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = std::process::Command::new(cargo)
        .args([
            "build",
            "--bin",
            "free-domain-browser-cef",
            "--bin",
            "free_domain_browser_cef_helper",
            "--target-dir",
            "target/bootstrap",
        ])
        .status()?;
    if !status.success() {
        anyhow::bail!("failed to build CEF binaries");
    }

    let app = bundle(
        Path::new("target/bundle"),
        Path::new("target/bootstrap/debug"),
        "free-domain-browser-cef",
        "free_domain_browser_cef_helper",
        None,
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
    let mut command = std::process::Command::new(&executable);
    command.args(args);

    // Put the CEF app in its own process group so Ctrl-C can take down the
    // browser and its helper processes together. CEF's message loop ignores
    // SIGINT, so the child must be terminated explicitly.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                libc::setpgid(0, 0);
                Ok(())
            });
        }
    }

    let mut child = command.spawn()?;
    let child_pid = child.id() as i32;
    ctrlc::set_handler(move || {
        #[cfg(unix)]
        unsafe {
            libc::kill(-child_pid, libc::SIGTERM);
            std::thread::sleep(std::time::Duration::from_millis(400));
            libc::kill(-child_pid, libc::SIGKILL);
        }
        std::process::exit(130);
    })?;
    let status = child.wait()?;
    std::process::exit(status.code().unwrap_or(1));
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("The bundle helper currently targets macOS only.");
}
