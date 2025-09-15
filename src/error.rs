use axum::{
    http::StatusCode, http::header::InvalidHeaderName, http::header::InvalidHeaderValue,
    response::IntoResponse, response::Response,
};
use tracing::error;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("libvips error")]
    Vips(libvips::error::Error, String),
    #[error(transparent)]
    TokioJoin(#[from] tokio::task::JoinError),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    InvalidHeaderName(#[from] InvalidHeaderName),
    #[error(transparent)]
    InvalidHeaderValue(#[from] InvalidHeaderValue),
    #[error(transparent)]
    UrlParse(#[from] url::ParseError),
    #[error("invalid backend")]
    InvalidBackend,
    #[error("io error")]
    Io(String),
    #[error("invalid signature")]
    InvalidSignature,
    #[error("file not found")]
    NotFound,
    #[error("rayon error: {0}")]
    Rayon(String),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        match self {
            Error::NotFound => StatusCode::NOT_FOUND.into_response(),
            Error::InvalidSignature => StatusCode::UNAUTHORIZED.into_response(),
            Error::Vips(err, error_buffer) => {
                error!(error = %err, detail = error_buffer);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
            Error::Io(err) => {
                error!(error = %err, detail = "io error");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
            Error::TokioJoin(err) => {
                error!("tokio error: {}", err);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
            _ => {
                error!("unknown error: {}", self);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    #[test]
    fn test_not_found_error_response() {
        let error = Error::NotFound;
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_invalid_signature_error_response() {
        let error = Error::InvalidSignature;
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_vips_error_response() {
        let vips_error = libvips::error::Error::OperationError("test error");
        let error = Error::Vips(vips_error, "error details".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_io_error_response() {
        let error = Error::Io("file read failed".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn test_tokio_join_error_response() {
        // Create a cancelled task to get a JoinError
        let handle = tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        });
        handle.abort();

        // This will create a JoinError when we try to await the cancelled task
        let join_error = handle.await.unwrap_err();

        let error = Error::TokioJoin(join_error);
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_http_error_response() {
        // Create a reqwest error by making an invalid URL request
        let http_error = reqwest::Client::new()
            .get("invalid_url")
            .build()
            .unwrap_err();
        let error = Error::Http(http_error);
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_invalid_backend_error_response() {
        let error = Error::InvalidBackend;
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_rayon_error_response() {
        let error = Error::Rayon("thread pool error".to_string());
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_url_parse_error_response() {
        let url_error = url::ParseError::EmptyHost;
        let error = Error::UrlParse(url_error);
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
