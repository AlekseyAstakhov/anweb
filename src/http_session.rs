use crate::request::{Request, RequestError};
use crate::tcp_session::{InnerTcpSession, ContentIsRead};
use crate::websocket;
use crate::websocket_session::{WebsocketSession, WebsocketError, WebsocketResult};
use std::io;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;
use crate::response::Response;

/// This comes with an callback of HTTP request for create a response or close of client socket.
#[derive(Clone)]
pub struct HttpSession {
    pub(crate) inner: Arc<InnerTcpSession>,
}

impl HttpSession {
    /// Client id on server in connection order.
    pub fn id(&self) -> u64 {
        self.inner.id()
    }

    /// An internet socket address, either IPv4 or IPv6.
    pub fn addr(&self) -> &SocketAddr {
        &self.inner.addr
    }

    /// Return response builder.
    pub fn response(&self, code: u16) -> Response {
        Response::new(code, &self)
    }

    /// Send raw data.
    pub fn response_raw(&self, data: &[u8]) {
        self.inner.send(data);
    }

    /// Send raw shared data.
    pub fn response_raw_arc(&self, data: &Arc<Vec<u8>>) {
        self.inner.send_arc(data);
    }

    /// Read raw http content (this is what is after headers).
    pub fn read_content(&self, callback: impl FnMut(&[u8], ContentIsRead, HttpSession) -> Result<(), Box<dyn std::error::Error>> + Send + 'static) {
        if let Ok(mut content_callback) = self.inner.content_callback.lock() {
            *content_callback = Some(Box::new(callback));
        }
    }

    /// Begin work with websocket. Contains vector with response to handshake and custom data, if the vector is empty then server will make handshake itself.
    pub fn accept_websocket(&mut self, request: &Request, payload: Vec<u8>, callback: impl FnMut(WebsocketResult, WebsocketSession) -> Result<(), WebsocketError> + Send + 'static) -> Result<WebsocketSession, io::Error> {
        if payload.is_empty() {
            match websocket::handshake_response(request) {
                Ok(response) => {
                    self.inner.send(&response);
                }
                Err(_) => {
                    return Err(io::Error::new(ErrorKind::Other, "Websocket handshake error"));
                }
            }
        } else {
            self.inner.send(&payload);
        }

        if let Ok(mut websocket_callback) = self.inner.websocket_callback.lock() {
            *websocket_callback = Some(Box::new(callback));
        }

        Ok(WebsocketSession { inner: self.inner.clone() })
    }

    /// Close of client socket. After clossing will be generated `sever::Event::Disconnected`.
    pub fn disconnect(&self) {
        self.inner.disconnect()
    }

    /// Prepared rfc7231 string for http responses, update once per second.
    pub fn http_date_string(&self) -> String {
        if let Ok(http_date_string) = self.inner.http_date_string.read() {
            http_date_string.clone()
        } else {
            String::new()
        }
    }
}

/// Received HTTP request or error receiving it.
pub type HttpResult<'a> = Result<&'a Request, HttpError>;

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
