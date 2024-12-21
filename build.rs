extern crate winres;

fn main() {
    println!("cargo:rustc-link-search=lib");
    println!("cargo:rustc-link-lib=static=LoaderFFI");
    println!("cargo:rustc-link-lib=static=loader");
    println!("cargo:rustc-link-lib=static=cimgui");

    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        res.set("FileVersion", env!("CARGO_PKG_VERSION"));
        res.set("LegalCopyright", "2024 Eigeen");
        res.set(
            "OriginalFilename",
            &format!("{}.dll", env!("CARGO_PKG_NAME")),
        );
        res.set("ProductName", env!("CARGO_PKG_NAME"));
        res.set("ProductVersion", env!("CARGO_PKG_VERSION"));

        res.compile().unwrap();
    }
}
