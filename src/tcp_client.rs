use crate::http_result::HttpError;
use crate::request::{ConnectionType, HttpVersion, Request, RequestError, ReceivedRequest};
use crate::request_parser::{ParseHttpRequestSettings, Parser};
use crate::tcp_session::TcpSession;
use crate::websocket;
use crate::websocket_session::WebsocketError;
use std::sync::atomic::Ordering;

/// Read, accumulate and process incoming data from clients. Parse http, websockets, tls and etc.
pub struct TcpClient {
    /// The framework user is using this.
    pub(crate) tcp_session: TcpSession,

    /// Parser with accumulation data.
    request_parser: Parser,

    /// Number of bytes of content that should be loaded with the http request.
    content_len: usize,
    /// Number of already read bytes of content.
    already_read_content_len: usize,

    /// It's used if connection upgraded to websocket. The parser need to be recreated only after error!
    websocket_parser: websocket::Parser,

    /// For limit of requests count in one socket read operation.
    pipelining_http_requests_count: u16,
}

impl TcpClient {
    pub fn new(tcp_session: TcpSession) -> Self {
        TcpClient {
            tcp_session,
            request_parser: Parser::new(),
            content_len: 0,
            already_read_content_len: 0,
            websocket_parser: websocket::Parser::new(),
            pipelining_http_requests_count: 0,
        }
    }

    pub fn on_read_ready(&mut self, settings: &Settings, read_buf: &mut [u8]) {
        self.pipelining_http_requests_count = 0;

        match self.tcp_session.inner.read(read_buf) {
            Ok(read_cnt) => {
                if read_cnt == 0 {
                    self.tcp_session.disconnect();
                    return;
                }

                self.process_data(&read_buf[..read_cnt], settings);
            }
            Err(err) => {
                if err.kind() == std::io::ErrorKind::WouldBlock {
                } else if err.kind() == std::io::ErrorKind::ConnectionReset {
                    self.tcp_session.disconnect();
                } else {
                    if self.tcp_session.is_http_mode() {
                        self.tcp_session.call_http_callback(Err(HttpError::StreamError(err)));
                    } else {
                        self.tcp_session.call_websocket_callback(Err(WebsocketError::StreamError(err)));
                    }

                    self.tcp_session.disconnect();
                }
            }
        }
    }

    fn process_data(&mut self, data: &[u8], settings: &Settings) {
        if self.tcp_session.need_disconnect() {
            return;
        }

        let mut http = true;
        if let Ok(callback) = self.tcp_session.inner.websocket_callback.lock() {
            if callback.is_some() {
                http = false;
            }
        }

        if http {
            let content_callback = self.tcp_session.inner.content_callback.lock()
                .unwrap_or_else(|err| { unreachable!(err) });
            let parse_request = content_callback.is_none();
            drop(content_callback); // unlock

            if parse_request {
                self.parse_request(data, settings);
            } else {
                self.read_content(data, settings);
            }
        } else {
            self.on_websocket_read(data, settings);
        }
    }

    fn parse_request(&mut self, data: &[u8], settings: &Settings) {
        self.pipelining_http_requests_count += 1;
        if self.pipelining_http_requests_count > settings.parse_http_request_settings.pipelining_requests_limit {
            self.tcp_session.call_http_callback(Err(HttpError::ParseRequestError(RequestError::PipeliningRequestsLimit)));
            self.tcp_session.disconnect();
            return;
        }

        match self.request_parser.parse_yet(data, &settings.parse_http_request_settings) {
            Ok((received_request, surplus)) => {
                self.process_received_request(received_request, surplus, settings);
            }
            Err(parse_err) => {
                match parse_err {
                    RequestError::Partial => {
                    }
                    parse_err => {
                        self.tcp_session.call_http_callback(Err(HttpError::ParseRequestError(parse_err)));
                        // close anyway
                        self.tcp_session.disconnect();
                    }
                }
            }
        }
    }

    fn process_received_request(&mut self, received_request: ReceivedRequest, surplus: Vec<u8>, settings: &Settings) {
        let need_disconnect_after_response = need_close_by_version_and_connection(&received_request);
        self.tcp_session.inner.need_disconnect_after_http_response.store(need_disconnect_after_response, Ordering::SeqCst);

        let content_len = received_request.content_len();

        self.tcp_session.call_http_callback(Ok(Request { received_request, tcp_session: self.tcp_session.clone() }));

        let content_callback = self.tcp_session.inner.content_callback.lock()
            .unwrap_or_else(|err| { unreachable!(err) });

        if content_callback.is_some() {
            if let Some(content_len) = content_len {
                self.content_len = content_len;
                self.already_read_content_len = 0;
            } else {
                self.tcp_session.call_http_callback(Err(HttpError::TryLoadContentWhenNoContentLen));
            }
        }

        drop(content_callback); // unlock

        if let Ok(websocket_callback) = self.tcp_session.inner.websocket_callback.lock() {
            if websocket_callback.is_some() {
                if let Ok(mut http_request_callback) = self.tcp_session.inner.http_request_callback.lock() {
                    *http_request_callback = None;
                    self.tcp_session.inner.is_http_mode.store(false, Ordering::SeqCst);
                }
            }
        }

        if !surplus.is_empty() && !self.tcp_session.need_disconnect() {
            // here is recursion
            self.process_data(&surplus, settings);
        }
    }

    fn read_content(&mut self, data: &[u8], settings: &Settings) {
        let mut content_callback = self.tcp_session.inner.content_callback.lock()
            .unwrap_or_else(|err| { unreachable!(err) });

        let mid = self.content_len.checked_sub(self.already_read_content_len)
            .unwrap_or_else(|| unreachable!())
            .min(data.len());

        let (content, surplus) = data.split_at(mid);
        self.already_read_content_len += content.len();
        let complete = self.already_read_content_len >= self.content_len;

        if let Some((content_callback, request)) = &mut *content_callback {
            let request = if complete { request.take() } else { None };
            if content_callback(content, request).is_err() {
                self.tcp_session.disconnect();
            }
        }

        if self.tcp_session.need_disconnect() {
            return;
        }

        if complete {
            *content_callback = None;

            self.content_len = 0;
            self.already_read_content_len = 0;

            drop(content_callback); // unlock

            if !surplus.is_empty() {
                // here is recursion
                self.process_data(&surplus, settings);
            }
        }
    }

    fn  on_websocket_read(&mut self, data: &[u8], settings: &Settings) {
        match self.websocket_parser.parse_yet(data, settings.websocket_payload_limit) {
            Ok(result) => {
                if let Some((frame, surplus)) = result {
                    let frame_is_close = frame.is_close();
                    self.tcp_session.call_websocket_callback(Ok(&frame));

                    if frame_is_close {
                        self.tcp_session.disconnect();
                    } else if !surplus.is_empty() {
                        self.process_data(&surplus, settings); // here is recursion
                    }
                }
            }
            Err(err) => {
                self.tcp_session.call_websocket_callback(Err(WebsocketError::ParseFrameError(err)));
                self.tcp_session.disconnect();
            }
        }
    }
}

/// Settings of incoming data processing.
#[derive(Clone)]
pub struct Settings {
    /// Parser settings to be applied for new connections.
    pub parse_http_request_settings: ParseHttpRequestSettings,
    /// Limit of payload length in websocket frame.
    pub websocket_payload_limit: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            parse_http_request_settings: ParseHttpRequestSettings::default(),
            websocket_payload_limit: 16_000_000,
        }
    }
}

/// Determines whether to close the connection after responding by the content of the request.
fn need_close_by_version_and_connection(request: &ReceivedRequest) -> bool {
    if let Some(connection_type) = &request.connection_type() {
        if let ConnectionType::Close = connection_type {
            return true;
        }
    } else {
        // by default in HTTP/1.0 connection close but in HTTP/1.1 keep-alive
        if let HttpVersion::Http1_0 = request.version() {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let mut request = ReceivedRequest::new();
        request.version = HttpVersion::Http1_0;
        request.connection_type = Some(ConnectionType::Close);
        assert_eq!(need_close_by_version_and_connection(&request), true);

        request.version = HttpVersion::Http1_0;
        request.connection_type = Some(ConnectionType::KeepAlive);
        assert_eq!(need_close_by_version_and_connection(&request), false);

        // by default in HTTP/1.0 connection close
        request.version = HttpVersion::Http1_0;
        request.connection_type = None;
        assert_eq!(need_close_by_version_and_connection(&request), true);

        request.version = HttpVersion::Http1_1;
        request.connection_type = Some(ConnectionType::Close);
        assert_eq!(need_close_by_version_and_connection(&request), true);

        request.version = HttpVersion::Http1_1;
        request.connection_type = Some(ConnectionType::KeepAlive);
        assert_eq!(need_close_by_version_and_connection(&request), false);

        // by default in HTTP/1.1 connection keep-alive
        request.version = HttpVersion::Http1_1;
        request.connection_type = None;
        assert_eq!(need_close_by_version_and_connection(&request), false);
    }
}
