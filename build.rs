fn main() {
    // The Linux packages ship the CEF runtime next to the executable, so the
    // loader has to look there. The launcher also sets LD_LIBRARY_PATH; this
    // keeps the binary working when it is started directly.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN");
    }

    #[cfg(windows)]
    {
        let mut resource = winres::WindowsResource::new();
        resource.set_icon("icons/icon.ico");
        resource.set("ProductName", "Free Domain Browser");
        resource.set("FileDescription", "Free Domain Browser");
        resource.set("ProductVersion", env!("CARGO_PKG_VERSION"));
        resource.set(
            "LegalCopyright",
            "Copyright © Free Domain Browser contributors",
        );
        resource
            .compile()
            .expect("failed to compile Windows resources");
    }
}
