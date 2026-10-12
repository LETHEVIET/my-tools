fn main() {
    println!("cargo:rerun-if-changed=assets/my-tools.ico");
    #[cfg(target_os = "windows")]
    winresource::WindowsResource::new()
        .set_icon("assets/my-tools.ico")
        .set("ProductName", "My Tools")
        .set("FileDescription", "My Tools - desktop utility hub")
        .set("LegalCopyright", "My Tools contributors")
        .compile()
        .expect("Could not embed the My Tools Windows application icon");
}
