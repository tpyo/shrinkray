fn main() {
    #[cfg(target_os = "macos")]
    println!(r"cargo:rustc-link-search=native=/opt/homebrew/lib");
    if let Ok(path) = std::env::var("SHRINKRAY_LIB_PATH") {
        println!(r"cargo:rustc-link-search=native={path}");
    }
}
