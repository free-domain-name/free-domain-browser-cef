fn main() {
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
