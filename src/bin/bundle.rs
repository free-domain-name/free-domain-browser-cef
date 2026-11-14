//! Build and package the app bundle used by CEF on macOS.

fn resolve_cef_path() -> Option<std::path::PathBuf> {
    if let Some(value) = std::env::var_os("CEF_PATH") {
        return Some(std::path::PathBuf::from(value));
    }
    let local = std::path::PathBuf::from("cef-binary");
    local.exists().then_some(local)
}

fn looks_like_cef(dir: &std::path::Path) -> bool {
    dir.join("libcef.so").is_file()
        || dir.join("libcef.dll").is_file()
        || dir.join("Chromium Embedded Framework.framework").is_dir()
        || dir.join("libcef.dylib").is_file()
}

/// CEF_PATH may be a flat distribution root or the shared cache root containing
/// `<version>/<platform>/`. Return the directory that actually owns libcef.
fn resolve_cef_runtime_path() -> Option<std::path::PathBuf> {
    let root = resolve_cef_path()?;
    let root = root.canonicalize().unwrap_or(root);
    if looks_like_cef(&root) {
        return Some(root);
    }
    for entry in std::fs::read_dir(&root).ok()?.flatten() {
        let path = entry.path();
        if looks_like_cef(&path) {
            return Some(path);
        }
        if let Ok(children) = std::fs::read_dir(&path) {
            for child in children.flatten() {
                if looks_like_cef(&child.path()) {
                    return Some(child.path());
                }
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
/// Ad-hoc sign the assembled bundle. Replacing the executable/framework during
/// bundling invalidates the linker signature; without a fresh signature macOS
/// kills the process with "Code Signature Invalid". A Developer ID signature can
/// replace this later with `scripts/sign-release.sh`.
fn adhoc_sign(app: &std::path::Path) -> anyhow::Result<()> {
    let status = std::process::Command::new("codesign")
        .args(["--force", "--deep", "--sign", "-"])
        .arg(app)
        .status()?;
    if !status.success() {
        anyhow::bail!("codesign failed for {}", app.display());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn main() -> anyhow::Result<()> {
    use cef::build_util::mac::{bundle, BundleInfo};
    use semver::Version;
    use std::path::Path;

    // `cef::build_util::mac::bundle` asks `cef-dll-sys` for the distribution
    // root through the process environment. `cargo run` does not guarantee that
    // CEF_PATH is exported, so resolve the tracked default and export an
    // absolute path before bundling.
    if let Some(cef_path) = resolve_cef_runtime_path() {
        let cef_path = cef_path.canonicalize().unwrap_or(cef_path);
        unsafe {
            std::env::set_var("CEF_PATH", &cef_path);
        }
    }

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
    let release = std::env::args().any(|arg| arg == "--release");
    let bootstrap_dir = if release {
        "target/bootstrap/release"
    } else {
        "target/bootstrap/debug"
    };
    let cef_path = resolve_cef_path();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut build = std::process::Command::new(cargo);
    if let Some(cef_path) = cef_path.as_ref() {
        build.env("CEF_PATH", cef_path);
    }
    build.args([
        "build",
        "--bin",
        "free-domain-browser-cef",
        "--bin",
        "free_domain_browser_cef_helper",
        "--target-dir",
        "target/bootstrap",
    ]);
    if release {
        build.arg("--release");
    }
    let status = build.status()?;
    if !status.success() {
        anyhow::bail!("failed to build CEF binaries");
    }

    let app = bundle(
        Path::new("target/bundle"),
        Path::new(bootstrap_dir),
        "free-domain-browser-cef",
        "free_domain_browser_cef_helper",
        // `assets/mac/free-domain-browser-cef.icns` is copied into
        // Contents/Resources and referenced as CFBundleIconFile.
        Some(Path::new("assets").to_path_buf()),
        info,
    )?;
    adhoc_sign(&app)?;

    // `cargo run` should launch the app in this terminal (with logs), not just
    // leave a bundle behind. CEF still needs to be launched from inside the
    // .app, so execute the bundled binary directly.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--build-only") {
        return Ok(());
    }
    let executable = app
        .join("Contents/MacOS/free-domain-browser-cef")
        .canonicalize()?;
    // CEF resolves its embedded framework relative to the working directory on
    // macOS 15+. Launch from inside the .app so `cargo run` works from the repo.
    let mut command = std::process::Command::new(&executable);
    if let Some(directory) = executable.parent() {
        command.current_dir(directory);
    }
    command.args(args);
    if let Some(cef_path) = resolve_cef_runtime_path() {
        command.env("CEF_PATH", cef_path);
    }
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

#[cfg(target_os = "linux")]
fn main() -> anyhow::Result<()> {
    use std::path::Path;

    let release = std::env::args().any(|arg| arg == "--release");
    let bootstrap_dir = if release {
        "target/bootstrap/release"
    } else {
        "target/bootstrap/debug"
    };
    let cef_path = resolve_cef_path();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut build = std::process::Command::new(cargo);
    if let Some(cef_path) = cef_path.as_ref() {
        build.env("CEF_PATH", cef_path);
    }
    build.args([
        "build",
        "--bin",
        "free-domain-browser-cef",
        "--bin",
        "free_domain_browser_cef_helper",
        "--target-dir",
        "target/bootstrap",
    ]);
    if release {
        build.arg("--release");
    }
    let status = build.status()?;
    if !status.success() {
        anyhow::bail!("failed to build CEF binaries");
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--build-only") {
        return Ok(());
    }

    let executable = Path::new(bootstrap_dir).join("free-domain-browser-cef");
    if !executable.exists() {
        anyhow::bail!("missing {}", executable.display());
    }
    let mut command = std::process::Command::new(&executable);
    command.args(args);
    if let Some(cef_root) = resolve_cef_path() {
        command.env("CEF_PATH", &cef_root);
        let mut paths = Vec::new();
        if let Some(runtime) = resolve_cef_runtime_path() {
            command.env("CEF_PATH", &runtime);
            paths.push(runtime);
        }
        paths.push(cef_root);
        if let Some(existing) = std::env::var_os("LD_LIBRARY_PATH") {
            paths.extend(std::env::split_paths(&existing));
        }
        command.env("LD_LIBRARY_PATH", std::env::join_paths(paths)?);
    }

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

#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    use std::path::Path;

    let release = std::env::args().any(|arg| arg == "--release");
    let bootstrap_dir = if release {
        "target/bootstrap/release"
    } else {
        "target/bootstrap/debug"
    };
    let cef_path = resolve_cef_path();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut build = std::process::Command::new(cargo);
    if let Some(cef_path) = cef_path.as_ref() {
        build.env("CEF_PATH", cef_path);
    }
    build.args([
        "build",
        "--bin",
        "free-domain-browser-cef",
        "--bin",
        "free_domain_browser_cef_helper",
        "--target-dir",
        "target/bootstrap",
    ]);
    if release {
        build.arg("--release");
    }
    let status = build.status()?;
    if !status.success() {
        anyhow::bail!("failed to build CEF binaries");
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--build-only") {
        return Ok(());
    }

    let executable = Path::new(bootstrap_dir).join("free-domain-browser-cef.exe");
    if !executable.exists() {
        anyhow::bail!("missing {}", executable.display());
    }
    let mut command = std::process::Command::new(&executable);
    command.args(args);
    if let Some(cef_path) = resolve_cef_runtime_path() {
        command.env("CEF_PATH", cef_path);
    }
    let status = command.status()?;
    std::process::exit(status.code().unwrap_or(1));
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn main() {
    eprintln!("The bundle helper currently targets macOS, Linux, and Windows.");
}
