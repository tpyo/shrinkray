pub mod config;
pub mod image;
pub mod options;

use libvips::VipsApp;
use once_cell::sync::OnceCell;

pub fn create_vips_app() -> &'static VipsApp {
    // libvips requires global initialization and assumes there is only
    // one global VipsApp per process. Creating multiple instances of
    // VipsApp::new(...) in the same test binary (even across different
    // tests) will lead to undefined behavior.
    static VIPS: OnceCell<VipsApp> = OnceCell::new();
    VIPS.get_or_init(|| {
        let app = VipsApp::new("shrinkray", false).expect("failed to initialize libvips");
        app.cache_set_max(0);
        app.cache_set_max_mem(0);
        app
    })
}
