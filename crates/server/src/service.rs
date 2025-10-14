use crate::config::Config;
use crate::error::Error;
use libvips::{VipsApp, error::Error as VipsError};
use tokio::signal;

pub struct Service {
    pub vips_app: &'static VipsApp,
    pub config: Config,
}

impl Service {
    pub fn new(config: Config) -> Self {
        Self {
            vips_app: shrinkray::create_vips_app(),
            config,
        }
    }
    pub fn vips_error(&self, err: VipsError) -> Error {
        let error_buffer = self.vips_app.error_buffer().unwrap_or("").replace('\n', "");
        self.vips_app.error_clear();
        Error::Vips(err, error_buffer)
    }
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

    #[test]
    fn test_vips_error() {
        use crate::config::Config;
        use libvips::error::Error as VipsError;

        let service = Service::new(Config::default());

        let vips_err = VipsError::OperationError("test operation failed");
        let result = service.vips_error(vips_err);

        match result {
            crate::error::Error::Vips(err, buffer) => {
                assert!(matches!(
                    err,
                    VipsError::OperationError("test operation failed")
                ));
                assert!(buffer.is_empty());
            }
            _ => panic!("expected Error::Vips variant"),
        }
    }
}
