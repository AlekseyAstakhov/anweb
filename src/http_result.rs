use crate::request::{Request, RequestError};

/// Received HTTP request or some error.
pub type HttpResult = Result<Request, HttpError>;

/// Http client errors.
#[derive(Debug)]
pub enum HttpError {
    /// Stream read/write error.
    StreamError(std::io::Error),
    /// When parse HTTP
    ParseRequestError(RequestError),
    /// When user wanna load content from http request but no content there.
    TryLoadContentWhenNoContentLen,
}

impl From<std::io::Error> for HttpError {
    fn from(err: std::io::Error) -> Self {
        HttpError::StreamError(err)
    }
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for HttpError {}
