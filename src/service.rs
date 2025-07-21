use crate::config::Config;
use crate::error::Error;
use libvips::{VipsApp, error::Error as VipsError};
use once_cell::sync::OnceCell;
use tokio::signal;

pub struct Service {
    pub vips_app: &'static VipsApp,
    pub config: Config,
}

impl Service {
    pub fn new(config: Config) -> Self {
        Self {
            vips_app: create_vips_app(),
            config,
        }
    }
    pub fn vips_error(&self, err: VipsError) -> Error {
        let error_buffer = self.vips_app.error_buffer().unwrap_or("").replace('\n', "");
        self.vips_app.error_clear();
        Error::Vips(err, error_buffer)
    }
}

fn create_vips_app() -> &'static VipsApp {
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

pub async fn shutdown() {
    let sigint = async {
        signal::ctrl_c()
            .await
            .expect("failed to create interrupt handler");
    };

    #[cfg(unix)]
    let sigterm = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to create terminate handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    #[cfg(unix)]
    tokio::select! {
        () = sigint => {
            tracing::info!("received interrupt signal");
        },
        () = sigterm => {
            tracing::info!("received terminate signal");
        },
    }

    #[cfg(not(unix))]
    tokio::select! {
        () = sigint => {
            tracing::info!("received interrupt signal");
        },
        () = terminate => {
            // On non-Unix platforms, wait on a pending future so the select!
            // branch compiles even though there is no SIGTERM.
            tracing::info!("received terminate signal");
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_shutdown_ctrl_c() {
        // This test checks that shutdown returns when ctrl_c is triggered.
        // Since we can't easily send a real ctrl_c in tests, we just ensure it doesn't hang.
        let shutdown_future = shutdown();
        let result = timeout(Duration::from_millis(100), shutdown_future).await;
        assert!(
            result.is_err(),
            "shutdown should wait for signal and timeout"
        );
    }
}
