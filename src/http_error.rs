use crate::request::RequestError;

/// Http client errors.
#[derive(Debug)]
pub enum HttpError {
    /// Read from sock error.
    ReadError(std::io::Error),
    /// Error of parsing data.
    ParseRequestError(RequestError),
    /// Write to sock error.
    WriteError(std::io::Error),
}

impl From<std::io::Error> for HttpError {
    fn from(err: std::io::Error) -> Self {
        HttpError::ReadError(err)
    }
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for HttpError {}
