use crate::cookie::Cookie;
use crate::request::{Request, RequestError};
use crate::response;
use crate::tcp_session::{InnerTcpSession, ContentIsRead};
use crate::websocket;
use crate::websocket_session::{WebsocketSession, WebsocketError, WebsocketResult};
use std::io;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::sync::Arc;

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

    /// Send response to client with status "200 OK" and text body.
    pub fn response_200_text(&self, text: &str, request: &Request) {
        self.response_raw(&response::text_200(text, request, &self.http_date_string()));
    }

    /// Send response to client with status "200 OK" and HTML body.
    pub fn response_200_html(&self, body: &str, request: &Request) {
        self.response_raw(&response::html_200(body, request, &self.http_date_string()));
    }

    pub fn response_200_wasm(&self, wasm_data: &[u8], request: &Request) {
        self.response_raw(&response::wasm_200(wasm_data, request, &self.http_date_string()));
    }

    pub fn response_200_html_with_cookie(&self, body: &str, cookie: &Cookie, request: &Request) {
        self.response_raw(&response::html_with_cookie_200(body, cookie, request, &self.http_date_string()));
    }

    pub fn response_404_empty(&self, request: &Request) {
        self.response_raw(&response::empty_404(request, &self.http_date_string()));
    }

    pub fn response_404_text(&self, text: &str, request: &Request) {
        self.response_raw(&response::text_404(text, request, &self.http_date_string()));
    }

    pub fn response_404_html(&self, html: &str, request: &Request) {
        self.response_raw(&response::html_404(html, request, &self.http_date_string()));
    }

    pub fn response_422_empty(&self, request: &Request) {
        self.response_raw(&response::unprocessable_entity_empty_422(request, &self.http_date_string()));
    }

    pub fn response_422_text(&self, text: &str, request: &Request) {
        self.response_raw(&response::unprocessable_entity_with_text_422(text, request, &self.http_date_string()));
    }

    pub fn response_400_empty(&self, request: &Request) {
        self.response_raw(&response::empty_bad_request_400(request, &self.http_date_string()));
    }

    pub fn response_400_text(&self, text: &str, request: &Request) {
        self.response_raw(&response::bad_request_with_text_400(text, request, &self.http_date_string()));
    }

    pub fn response_303_with_cookie(&self, path: &str, cookie: &Cookie, request: &Request) {
        self.response_raw(&response::redirect_303_with_cookie(path, cookie, request, &self.http_date_string()));
    }

    pub fn empty_500(&self, request: &Request) {
        self.response_raw(&response::empty_500(request, &self.http_date_string()));
    }

    pub fn text_500(&self, text: &str, request: &Request) {
        self.response_raw(&response::text_500(text, request, &self.http_date_string()));
    }

    pub fn html_500(&self, html: &str, request: &Request) {
        self.response_raw(&response::html_500(html, request, &self.http_date_string()));
    }

    /// Prepared rfc7231 string for http responses, update once per second.
    pub fn http_date_string(&self) -> String {
        if let Ok(http_date_string) = self.inner.http_date_string.read() {
            http_date_string.clone()
        } else {
            String::new()
        }
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
