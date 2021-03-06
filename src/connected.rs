use crate::content_loader::ContentLoader;
use crate::http_client::HttpError;
use crate::request::{ConnectionType, HttpVersion, Request, RequestError};
use crate::request_parser::{ParseHttpRequestSettings, Parser};
use crate::tcp_client::TcpClient;
use crate::websocket;
use crate::websocket_client::WebsocketError;
use std::sync::atomic::Ordering;

/// Read, accumulate and process incoming data from clients. Parse http, websockets, tls and etc.
pub struct Connected {
    /// The framework user is using this.
    pub(crate) client: TcpClient,

    /// Parser with accumulation data.
    request_parser: Parser,

    /// Parser with accumulation data.
    content_loader: Option<ContentLoader>,

    /// It's used if connection upgraded to websocket. The parser need to be recreated only after error!
    websocket_parser: websocket::Parser,

    /// For limit of requests count in one socket read operation.
    pipelining_http_requests_count: u16,
}

impl Connected {
    pub fn new(client: TcpClient) -> Self {
        Connected {
            client,
            request_parser: Parser::new(),
            content_loader: None,
            websocket_parser: websocket::Parser::new(),
            pipelining_http_requests_count: 0,
        }
    }

    pub fn on_ready(&mut self, settings: &Settings) {
        self.pipelining_http_requests_count = 0;

        const BUF_SIZE: usize = 1024;
        let mut tmp_buf = [0; BUF_SIZE];

        match self.client.inner.read(&mut tmp_buf) {
            Ok(read_cnt) => {
                if read_cnt == 0 {
                    self.client.disconnect();
                    return;
                }

                self.process_read(&tmp_buf[..read_cnt], settings);
            }
            Err(err) => {
                if err.kind() == std::io::ErrorKind::WouldBlock {
                } else if err.kind() == std::io::ErrorKind::ConnectionReset {
                    self.client.disconnect();
                } else {
                    if self.client.is_http_mode() {
                        self.client.call_http_callback(Err(HttpError::StreamError(err)));
                    } else {
                        self.client.call_websocket_callback(Err(WebsocketError::StreamError(err)));
                    }

                    self.client.disconnect();
                }
            }
        }
    }

    fn process_read(&mut self, tmp_buf: &[u8], settings: &Settings) {
        if self.client.need_disconnect() {
            return;
        }

        let mut tls = true;
        if let Ok(callback) = self.client.inner.websocket_callback.lock() {
            if callback.is_some() {
                tls = false;
            }
        }

        if tls {
            let parse_request = if let Ok(content_callback) = self.client.inner.raw_content_callback.lock() {
                content_callback.is_none()
            } else {
                true
            };

            if parse_request {
                self.parse_request(tmp_buf, settings);
            } else {
                self.read_content(tmp_buf, settings);
            }
        } else {
            self.on_websocket_read(tmp_buf, settings);
        }
    }

    fn parse_request(&mut self, tmp_buf: &[u8], settings: &Settings) {
        self.pipelining_http_requests_count += 1;
        if self.pipelining_http_requests_count > settings.parse_http_request_settings.pipelining_requests_limit {
            self.client.call_http_callback(Err(HttpError::ParseRequestError(RequestError::PipeliningRequestsLimit)));
            self.client.disconnect();
            return;
        }

        match self.request_parser.parse_yet(tmp_buf, &settings.parse_http_request_settings) {
            Ok(surplus) => {
                self.process_request(surplus, settings);
            }
            Err(parse_err) => {
                match parse_err {
                    RequestError::Partial => {
                    }
                    parse_err => {
                        self.client.call_http_callback(Err(HttpError::ParseRequestError(parse_err)));
                        // close anyway
                        self.client.disconnect();
                    }
                }
            }
        }
    }

    fn process_request(&mut self, surplus: Vec<u8>, settings: &Settings) {
        let need_disconnect_after_response = need_close_by_version_and_connection(&self.request_parser.request);
        self.client.inner.need_disconnect_after_http_response.store(need_disconnect_after_response, Ordering::SeqCst);

        self.client.call_http_callback(Ok(&self.request_parser.request));

        if let Ok(content_callback) = self.client.inner.raw_content_callback.lock() {
            if content_callback.is_some() {
                if let Some(content_len) = self.request_parser.request.content_len {
                    self.content_loader = Some(ContentLoader::new(content_len));
                } else {
                    self.client.call_http_callback(Err(HttpError::TryLoadContentWhenNoContentLen));
                }
            }
        }

        if let Ok(websocket_callback) = self.client.inner.websocket_callback.lock() {
            if websocket_callback.is_some() {
                if let Ok(mut http_request_callback) = self.client.inner.http_request_callback.lock() {
                    *http_request_callback = None;
                    self.client.inner.is_http_mode.store(false, Ordering::SeqCst);
                }
            }
        }

        self.request_parser.restart();

        if !surplus.is_empty() && !self.client.need_disconnect() {
            // here is recursion
            self.process_read(&surplus, settings);
        }
    }

    fn read_content(&mut self, tmp_buf: &[u8], settings: &Settings) {
        if let Some(content_loader) = &mut self.content_loader {
            if let Some((content, surplus)) = content_loader.load_yet(tmp_buf) {
                // Loaded!
                self.client.call_raw_content_callback(content);
                if let Ok(mut callback) = self.client.inner.raw_content_callback.lock() {
                    *callback = None;
                }
                self.content_loader = None;

                if !surplus.is_empty() {
                    // here is recursion
                    self.process_read(&surplus, settings);
                }
            }
        }
    }

    fn on_websocket_read(&mut self, tmp_buf: &[u8], settings: &Settings) {
        match self.websocket_parser.parse_yet(tmp_buf, settings.websocket_payload_limit) {
            Ok(result) => {
                if let Some((frame, surplus)) = result {
                    let frame_is_close = frame.is_close();
                    self.client.call_websocket_callback(Ok(&frame));

                    if frame_is_close {
                        self.client.disconnect();
                    } else if !surplus.is_empty() {
                        self.process_read(&surplus, settings); // here is recursion
                    }
                }
            }
            Err(err) => {
                self.client.call_websocket_callback(Err(WebsocketError::ParseFrameError(err)));
                self.client.disconnect();
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
fn need_close_by_version_and_connection(request: &Request) -> bool {
    if let Some(connection_type) = &request.connection_type {
        if let ConnectionType::Close = connection_type {
            return true;
        }
    } else {
        // by default in HTTP/1.0 connection close but in HTTP/1.1 keep-alive
        if let HttpVersion::Http1_0 = request.version {
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
        let mut request = Request::new();
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
