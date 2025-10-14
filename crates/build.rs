fn main() {
    #[cfg(target_os = "macos")]
    let default_lib_path = "/opt/homebrew/lib";
    #[cfg(target_os = "linux")]
    let default_lib_path = "/opt/shrinkray/lib";

    let lib_path = std::env::var("LIB_PATH").unwrap_or(default_lib_path.to_string());
    println!(r"cargo:rustc-link-search=native={lib_path}");
}

