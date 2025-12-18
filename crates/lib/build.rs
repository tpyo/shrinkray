fn main() {
    #[cfg(target_os = "macos")]
    println!(r"cargo:rustc-link-search=native=/opt/homebrew/lib");

    if let Ok(path) = std::env::var("SHRINKRAY_LIB_PATH") {
        println!(r"cargo:rustc-link-search=native={path}");
    }

    if let Some((major, minor, patch)) = detect_libvips_version() {
        // libvips version 8.17.0 introduced a breaking change to
        // profile parameter names used in thumbnailing functions.
        // See https://github.com/libvips/libvips/pull/4488
        if major < 8 || (major == 8 && minor < 17) {
            panic!(
                "\nERROR:\nlibvips version 8.17.0 or higher is required for shrinkray v1.0.2+, for older libvips versions, use shrinkray v1.0.1.\n"
            );
        }
    } else {
        println!("cargo:warning=libvips version could not be detected, skipping version check.");
    }
}

fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let s = s.trim_start_matches('v');
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() >= 3 {
        let major = parts[0].parse().ok()?;
        let minor = parts[1].parse().ok()?;
        let patch = parts[2].parse().ok()?;
        Some((major, minor, patch))
    } else {
        None
    }
}

fn detect_libvips_version() -> Option<(u32, u32, u32)> {
    // Detect system libvips version
    if let Ok(output) = std::process::Command::new("pkg-config")
        .args(["--modversion", "vips"])
        .output()
        && output.status.success()
        && let Ok(version_str) = String::from_utf8(output.stdout)
    {
        let version = version_str.trim();
        return parse_version(version);
    }
    None
}
