use crate::cookie::{parse_cookie, CookieOfRequst};
use crate::query::{parse_query, Query};
use std::str::from_utf8;
use crate::tcp_session::{InnerTcpSession, ContentIsComplite};
use std::sync::Arc;
use crate::websocket_session::{WebsocketSession, WebsocketResult, WebsocketError};
use crate::websocket;
use std::io::ErrorKind;
use crate::response::Response;
use std::net::SocketAddr;
use std::io;

#[derive(Clone)]
pub struct ParsedRequest {
    /// Raw buffer of request without content.
    pub(crate) raw: Vec<u8>,
    /// Indices of method in raw buffer ('raw').
    pub(crate) method_end_index: usize,
    /// Indices of path in raw buffer ('raw').
    pub(crate) path_indices: (usize, usize),
    /// Indices of query in raw buffer ('raw').
    pub(crate) raw_query_indices: (usize, usize),

    /// Version "HTTP/1.0" or "HTTP/1.1".
    pub(crate) version: HttpVersion,
    /// Headers.
    pub(crate) headers: Vec<Header>,

    /// Value of header "Connection: keep-alive/close", if no header then None
    pub(crate) connection_type: Option<ConnectionType>,
    /// Value of header "Content-length", if no header then None.
    pub(crate) content_len: Option<usize>,

    /// Need for return $str from path() function
    pub(crate) decoded_path: String,
}

/// HTTP request like "GET /?abc=123 HTTP/1.1\r\nConnection: keep-alive\r\n\r\n".
pub struct Request {
    pub(crate) parsed_request: ParsedRequest,
    pub(crate) tcp_session: Arc<InnerTcpSession>,
}

/// Parsed header.
#[derive(Debug, Clone)]
pub struct Header {
    /// Name.
    pub name: String,
    /// Value.
    pub value: String,
}

impl std::fmt::Display for Header {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.write_str(&self.name)?;
        fmt.write_str(": ")?;
        fmt.write_str(&self.value)?;
        fmt.write_str("\r\n")?;
        Ok(())
    }
}

/// Connection type specified in HTTP request as Connection: keep-alive, Connection: close.
#[derive(Debug, Clone)]
pub enum ConnectionType {
    KeepAlive,
    Close,
}

#[derive(Debug, Clone, Eq, PartialEq)]
/// Supported http protocol versions.
pub enum HttpVersion {
    Http1_0,
    Http1_1,
}

#[derive(Debug, Clone)]
/// Request is not full or parse request error or limit some request content.
pub enum RequestError {
    Partial,

    RequestLine,

    MethodLenLimit,
    PathLenLimit,
    QueryLenLimit,
    WrongVersion,
    UnsupportedProtocol,
    WrongHeader,
    EmptyHeaderName,
    VersionLenLimit,
    HeadersCountLimit,
    HeaderNameLenLimit,
    HeaderValueLenLimit,
    PipeliningRequestsLimit,
    ContentLengthLimit,
    ContentLengthParseError,
}

impl ParsedRequest {
    /// Creates a request with undefined fields.
    pub fn new() -> Self {
        ParsedRequest {
            method_end_index: 0,
            path_indices: (0, 0),
            raw_query_indices: (0, 0),
            version: HttpVersion::Http1_0,
            headers: Vec::with_capacity(16),
            raw: Vec::with_capacity(64),
            connection_type: None,
            content_len: None,
            decoded_path: String::new(),
        }
    }
}

impl ParsedRequest {
    /// The method slice in request buffer converted to utf8 string. Empty if invalid utf8 string.
    pub fn method(&self) -> &str {
        if self.method_end_index > self.raw.len() {
            dbg!("unreachable code");
            return "";
        }

        from_utf8(&self.raw[0..self.method_end_index]).unwrap_or("")
    }

    /// Path. Decoded. Empty if no valid utf-8 or decoding error.
    pub fn path(&self) -> &str {
        return &self.decoded_path;
    }

    /// The parsed query to names and values array.
    pub fn query(&self) -> Query {
        parse_query(&self.raw_query())
    }

    /// Header value by name.
    pub fn header_value(&self, name: &str) -> Option<&str> {
        for header in self.headers.iter() {
            if header.name == name {
                return Some(&header.value);
            }
        }

        None
    }

    /// Version "HTTP/1.0" or "HTTP/1.1".
    pub fn version(&self) -> &HttpVersion {
        &self.version
    }
    /// Headers.
    pub fn headers(&self) -> &Vec<Header> {
        &self.headers
    }

    /// Value of header "Connection: keep-alive/close", if no header then None
    pub fn connection_type(&self) -> &Option<ConnectionType> {
        &self.connection_type
    }
    /// Value of header "Content-length", if no header then None.
    pub fn content_len(&self) -> Option<usize> {
        self.content_len
    }

    /// Cookies FROM FIRST HEADER "Cookie". RFC 6265, 5.4. "The Cookie Header: When the user agent generates an HTTP request, the user agent MUST NOT attach more than one Cookie header field".
    pub fn cookies(&self) -> Vec<CookieOfRequst> {
        if let Some(cookie_header) = self.header_value("Cookie") {
            return parse_cookie(&cookie_header);
        }

        Vec::new()
    }

    /// Check existence header Content-Len, Content-Type and type application/x-www-form-urlencoded.
    /// No check that method is necessarily "POST", "PUT" or "PATCH".
    pub fn has_post_form(&self) -> bool {
        if self.content_len.is_some() {
            if let Some(value) = self.header_value("Content-Type") {
                if value == "application/x-www-form-urlencoded" {
                    return true;
                }
            }
        }

        false
    }

    /// Raw buffer of request.
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }

    /// Method as raw bytes in request buffer.
    pub fn raw_method(&self) -> &[u8] {
        if self.method_end_index > self.raw.len() {
            dbg!("unreachable code");
            return b"";
        }

        &self.raw[0..self.method_end_index]
    }

    /// Path as raw bytes in request buffer.
    pub fn raw_path(&self) -> &[u8] {
        if self.path_indices.0 > self.path_indices.1 || self.path_indices.1 > self.raw.len() {
            dbg!("unreachable code");
            return b"";
        }

        &self.raw[self.path_indices.0..self.path_indices.1]
    }

    /// Query slice in request buffer. Empty if no query.
    pub fn raw_query(&self) -> &[u8] {
        if self.raw_query_indices.0 > self.raw_query_indices.1 || self.raw_query_indices.1 > self.raw.len() {
            dbg!("unreachable code");
            return b"";
        }

        &self.raw[self.raw_query_indices.0..self.raw_query_indices.1]
    }
}

impl Request {
    /// The method slice in request buffer converted to utf8 string. Empty if invalid utf8 string.
    pub fn method(&self) -> &str {
        self.parsed_request.method()
    }

    /// Path. Decoded. Empty if no valid utf-8 or decoding error.
    pub fn path(&self) -> &str {
        self.parsed_request.path()
    }

    /// The parsed query to names and values array.
    pub fn query(&self) -> Query {
        self.parsed_request.query()
    }

    /// Header value by name.
    pub fn header_value(&self, name: &str) -> Option<&str> {
        self.parsed_request.header_value(name)
    }

    /// Version "HTTP/1.0" or "HTTP/1.1".
    pub fn version(&self) -> &HttpVersion {
        self.parsed_request.version()
    }
    /// Headers.
    pub fn headers(&self) -> &Vec<Header> {
        &self.parsed_request.headers()
    }

    /// Value of header "Connection: keep-alive/close", if no header then None
    pub fn connection_type(&self) -> &Option<ConnectionType> {
        &self.parsed_request.connection_type()
    }
    /// Value of header "Content-length", if no header then None.
    pub fn content_len(&self) -> Option<usize> {
        self.parsed_request.content_len()
    }

    /// Cookies FROM FIRST HEADER "Cookie". RFC 6265, 5.4. "The Cookie Header: When the user agent generates an HTTP request, the user agent MUST NOT attach more than one Cookie header field".
    pub fn cookies(&self) -> Vec<CookieOfRequst> {
        self.parsed_request.cookies()
    }

    /// Check existence header Content-Len, Content-Type and type application/x-www-form-urlencoded.
    /// No check that method is necessarily "POST", "PUT" or "PATCH".
    pub fn has_post_form(&self) -> bool {
        self.parsed_request.has_post_form()
    }

    /// Raw buffer of request.
    pub fn raw(&self) -> &[u8] {
        self.parsed_request.raw()
    }

    /// Method as raw bytes in request buffer.
    pub fn raw_method(&self) -> &[u8] {
        self.parsed_request.raw_method()
    }

    /// Path as raw bytes in request buffer.
    pub fn raw_path(&self) -> &[u8] {
        self.parsed_request.raw_path()
    }

    /// Return reference to request data structure.
    pub fn parsed_reauest(&self) -> &ParsedRequest {
        &self.parsed_request
    }


    /// Query slice in request buffer. Empty if no query.
    pub fn raw_query(&self) -> &[u8] {
        self.parsed_request.raw_query()
    }

    /// Client id on server in connection order.
    pub fn id(&self) -> u64 {
        self.tcp_session.id()
    }

    /// Net address of client, either IPv4 or IPv6.
    pub fn addr(&self) -> &SocketAddr {
        &self.tcp_session.addr
    }

    /// Return response builder.
    pub fn response(&self, code: u16) -> Response {
        Response::new(code, &self)
    }

    /// Send raw data.
    pub fn response_raw(&self, data: &[u8]) {
        self.tcp_session.send(data);
    }

    /// Send raw shared data.
    pub fn response_raw_arc(&self, data: &Arc<Vec<u8>>) {
        self.tcp_session.send_arc(data);
    }

    /// Read raw http content (this is what is after headers).
    pub fn read_content(self, callback: impl FnMut(&[u8], ContentIsComplite) -> Result<(), Box<dyn std::error::Error>> + Send + 'static) {
        let tcp_session = self.tcp_session.clone();
        if let Ok(mut content_callback) = tcp_session.content_callback.lock() {
            *content_callback = Some((Box::new(callback), Some(self)));
        }
        drop(tcp_session);
    }

    /// Begin work with websocket. Contains vector with response to handshake and custom data, if the vector is empty then server will make handshake itself.
    pub fn accept_websocket(&mut self, payload: Vec<u8>, callback: impl FnMut(WebsocketResult, WebsocketSession) -> Result<(), WebsocketError> + Send + 'static) -> Result<WebsocketSession, io::Error> {
        if payload.is_empty() {
            match websocket::handshake_response(&self.parsed_request) {
                Ok(response) => {
                    self.tcp_session.send(&response);
                }
                Err(_) => {
                    return Err(io::Error::new(ErrorKind::Other, "Websocket handshake error"));
                }
            }
        } else {
            self.tcp_session.send(&payload);
        }

        if let Ok(mut websocket_callback) = self.tcp_session.websocket_callback.lock() {
            *websocket_callback = Some(Box::new(callback));
        }

        Ok(WebsocketSession { inner: self.tcp_session.clone() })
    }

    /// Close of client socket. After clossing will be generated `sever::Event::Disconnected`.
    pub fn disconnect(&self) {
        self.tcp_session.disconnect()
    }

    /// Prepared rfc7231 string for http responses, update once per second.
    pub fn rfc7231_date_string(&self) -> String {
        if let Ok(http_date_string) = self.tcp_session.http_date_string.read() {
            http_date_string.clone()
        } else {
            String::new()
        }
    }
}

impl HttpVersion {
    /// Return version as string. If version in request not supported server return HTTP/1.1 response.
    pub fn to_string_for_response(&self) -> &str {
        match self {
            HttpVersion::Http1_0 => "HTTP/1.0",
            HttpVersion::Http1_1 => "HTTP/1.1",
        }
    }
}
