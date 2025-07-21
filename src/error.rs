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
