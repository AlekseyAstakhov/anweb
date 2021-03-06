use crate::request::{ConnectionType, Header, HttpVersion, Request, RequestError};
use std::str::from_utf8;

/// HTTP request parser.
pub struct Parser {
    /// Not ready request. Internal state between parsing iterations.
    pub request: Request,
    /// What parse now. Internal state between parsing iterations.
    parse_state: ParseState,
}

/// What parse now. Internal state between parsing iterations.
#[derive(Debug)]
pub enum ParseState {
    Method,
    /// Path with start index.
    Path(usize),
    /// Query with start index.
    Query(usize),
    /// Query with start index.
    Version(usize),
    /// Header with start index and separator index. Separator ':'.
    Header(usize, usize),
}

const VERSION_LEN: usize = 8;

/// Parser settings to be applied for new connections.
#[derive(Debug, Clone)]
pub struct ParseHttpRequestSettings {
    /// Maximum of bytes in method. In request line.
    pub method_len_limit: u16,
    /// Maximum of bytes in path. In request line.
    pub path_len_limit: u16,
    /// Maximum of bytes in query without '?' in request line.
    pub query_len_limit: u16,
    /// Maximum number of headers.
    pub headers_count_limit: u16,
    /// Maximum of bytes in header name.
    pub header_name_len_limit: u16,
    /// Maximum of bytes in header value. Including optional ' '.
    pub header_value_len_limit: u16,
    /// Maximum of requests count in one socket read operation. Several requests in can come from the client only if he is in pipelining mode. The number of possible requests is still limited by the size of the read buffer. Between read operations, the request counter is reset to zero.
    pub pipelining_requests_limit: u16,
}

impl Parser {
    pub fn new() -> Self {
        Parser {
            parse_state: ParseState::Method,
            request: Request::new(),
        }
    }

    /// Parse. At the moment, in case of an error, the parser becomes invalid and needs to be recreated.
    pub fn parse_yet(&mut self, buf: &[u8], parse_settings: &ParseHttpRequestSettings) -> Result<Vec<u8>, RequestError> {
        let prev_idx = self.request.raw.len();
        self.request.raw.extend_from_slice(buf);

        let raw_buf = &self.request.raw;

        let mut request_len = None; // determines request end found
        for (i, ch) in raw_buf.iter().enumerate().skip(prev_idx) {
            match self.parse_state {
                ParseState::Method => match *ch {
                    b' ' => {
                        self.request.method_end_index = i;
                        self.parse_state = ParseState::Path(i + 1);
                    }
                    b'\n' => {
                        return Err(RequestError::RequestLine);
                    }
                    _ => {
                        if i >= parse_settings.method_len_limit as usize {
                            return Err(RequestError::MethodLenLimit);
                        }
                    }
                },
                ParseState::Path(path_index) => match *ch {
                    b' ' => {
                        self.request.path_indices = (path_index, i);
                        self.parse_state = ParseState::Version(i + 1);
                    }
                    b'\n' => {
                        return Err(RequestError::RequestLine);
                    }
                    b'?' => {
                        self.request.path_indices = (path_index, i);
                        self.parse_state = ParseState::Query(i + 1);
                    }
                    _ => {
                        if i - path_index >= parse_settings.path_len_limit as usize {
                            return Err(RequestError::PathLenLimit);
                        }
                    }
                },
                ParseState::Query(query_index) => match *ch {
                    b' ' => {
                        self.request.raw_query_indices = (query_index, i);
                        self.parse_state = ParseState::Version(i + 1);
                    }
                    b'\n' => {
                        return Err(RequestError::RequestLine);
                    }
                    _ => {
                        if i - query_index >= parse_settings.query_len_limit as usize {
                            return Err(RequestError::QueryLenLimit);
                        }
                    }
                },
                ParseState::Version(version_index) => match *ch {
                    b'\n' => match version_from_data(&raw_buf[version_index..i - 1]) {
                        Ok(ver) => {
                            self.request.version = ver;
                            self.parse_state = ParseState::Header(i + 1, 0);
                        }
                        Err(ver_err) => match ver_err {
                            VersionError::UnsupportedProtocol => return Err(RequestError::UnsupportedProtocol),
                            _ => return Err(RequestError::WrongVersion),
                        },
                    },
                    _ => {
                        if i as i32 - version_index as i32 > VERSION_LEN as i32 {
                            return Err(RequestError::VersionLenLimit);
                        }
                    }
                },
                ParseState::Header(header_index, header_separator_index) => {
                    // check end
                    if *ch == b'\n' && &raw_buf[i - 3..=i] == b"\r\n\r\n" {
                        request_len = Some(i + 1); // determines request end found
                        break;
                    }

                    // name limit check
                    if header_separator_index == 0 {
                        if i as i32 - header_index as i32 > parse_settings.header_name_len_limit as i32 {
                            return Err(RequestError::HeaderNameLenLimit);
                        }
                    }
                    // value limit check
                    else if i as i32 - header_separator_index as i32 > parse_settings.header_value_len_limit as i32 + 2 {
                        return Err(RequestError::HeaderValueLenLimit);
                    }

                    // From RFC 7230:
                    // Each header field consists of a case-insensitive field name followed by a colon (":"),
                    // optional leading whitespace, the field value, and optional trailing whitespace.
                    if *ch == b':' && header_separator_index == 0 {
                        // check here because need find "\r\n\r\n" above. If found ':' then no "\r\n\r\n"
                        if self.request.headers.len() >= parse_settings.headers_count_limit as usize {
                            return Err(RequestError::HeadersCountLimit);
                        }

                        // empty header name
                        if i <= header_index {
                            return Err(RequestError::EmptyHeaderName);
                        }

                        self.parse_state = ParseState::Header(header_index, i);
                    } else if *ch == b'\n' && &raw_buf[i - 1..=i] == b"\r\n" {
                        if header_separator_index == 0 || i as i32 - (header_separator_index as i32) < 2 {
                            return Err(RequestError::WrongHeader);
                        }

                        if header_separator_index <= header_index {
                            return Err(RequestError::WrongHeader);
                        }

                        let value_idx = if raw_buf[header_separator_index + 1] == b' ' { header_separator_index + 2 } else { header_separator_index + 1 };

                        if value_idx >= i - 1 {
                            return Err(RequestError::WrongHeader);
                        }

                        let header_name = from_utf8(&self.request.raw[header_index..header_separator_index]).unwrap_or("");
                        if header_name.is_empty() {
                            return Err(RequestError::WrongHeader);
                        }

                        let header_value = from_utf8(&self.request.raw[value_idx..i - 1]);
                        if header_value.is_err() {
                            return Err(RequestError::WrongHeader);
                        }
                        let header_value = header_value.unwrap_or("");

                        let header = Header {
                            name: header_name.to_string(),
                            value: header_value.to_string(),
                        };

                        // check "Contention" header
                        if self.request.connection_type.is_none() {
                            self.request.connection_type = self.header_is_connection_type(&header);
                        }

                        // check "Content-Length"  header
                        if self.request.content_len.is_none() {
                            self.request.content_len = self.header_is_content_length(&header)?;
                        }

                        self.request.headers.push(header);
                        self.parse_state = ParseState::Header(i + 1, 0);
                    }
                }
            }
        }

        // if request end found
        if let Some(request_len) = request_len {
            self.parse_state = ParseState::Method;

            let surplus = self.request.raw[request_len..].to_vec();
            self.request.raw.truncate(request_len);

            return Ok(surplus);
        }

        Err(RequestError::Partial)
    }

    pub fn restart(&mut self) {
        self.request = Request::new();
    }

    fn header_is_connection_type(&self, header: &Header) -> Option<ConnectionType> {
        if header.name == "Connection" {
            if header.value == "keep-alive" {
                return Some(ConnectionType::KeepAlive);
            } else if header.value == "close" {
                return Some(ConnectionType::Close);
            }
        }

        None
    }

    fn header_is_content_length(&self, header: &Header) -> Result<Option<usize>, RequestError> {
        if header.name == "Content-Length" {
            if let Ok(content_length) = header.value.parse() {
                return Ok(Some(content_length));
            } else {
                return Err(RequestError::ContentLengthParseError);
            }
        }

        Ok(None)
    }
}

enum VersionError {
    WrongLen,
    WrongText,
    UnsupportedProtocol,
}

fn version_from_data(data: &[u8]) -> Result<HttpVersion, VersionError> {
    if data.len() != VERSION_LEN {
        return Err(VersionError::WrongLen);
    }

    if &data[..5] != b"HTTP/" {
        return Err(VersionError::WrongText);
    }

    let ver = &data[5..];
    if ver == b"1.1" {
        return Ok(HttpVersion::Http1_1);
    } else if ver == b"1.0" {
        return Ok(HttpVersion::Http1_0);
    }

    Err(VersionError::UnsupportedProtocol)
}

impl Default for ParseHttpRequestSettings {
    fn default() -> Self {
        ParseHttpRequestSettings {
            method_len_limit: 7,
            path_len_limit: 512,
            query_len_limit: 512,
            // I googled that default limits for headers on other servers: Apache 8K, Nginx 4K-8K, IIS 8K-16K, Tomcat 8K – 48K. I don’t know yet why so many.
            headers_count_limit: 16,
            header_name_len_limit: 32,
            header_value_len_limit: 512,
            pipelining_requests_limit: 64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl PartialEq for Header {
        fn eq(&self, other: &Self) -> bool {
            self.name == other.name && self.value == other.value
        }
    }

    #[test]
    fn test() {
        let parse_settings = ParseHttpRequestSettings {
            method_len_limit: 7,
            path_len_limit: 512,
            query_len_limit: 512,
            headers_count_limit: 5,
            header_name_len_limit: 64,
            header_value_len_limit: 512,
            pipelining_requests_limit: 12,
        };

        let mut parser = Parser::new();
        let request_str = "GET / HTTP/1.1\r\nConnection: keep-alive\r\n\r\n";
        if let Ok(surplus) = parser.parse_yet(request_str.as_bytes(), &parse_settings) {
            assert!(surplus.is_empty());
        } else {
            assert!(false);
        }

        parser.restart();

        let request_str = "GET / HTTP/1.1\r\nConnection: keep-alive\r\n\r\naaa";
        if let Ok(surplus) = parser.parse_yet(request_str.as_bytes(), &parse_settings) {
            assert!(surplus.len() == 3);
        } else {
            assert!(false);
        }

        parser.restart();

        let request_str = "GET /index HTTP/1.1\r\n\r\n";
        if let Ok(_) = parser.parse_yet(request_str.as_bytes(), &parse_settings) {
            let request = &parser.request;
            assert_eq!(request.method(), "GET");
            assert_eq!(request.path(), "/index");
            assert_eq!(request.raw_query(), b"");
            assert_eq!(request.version, HttpVersion::Http1_1);
            assert!(request.headers.is_empty());
        } else {
            assert!(false);
        }

        parser.restart();

        let request_str = "POST /index?a=1&b=2;c=3 HTTP/1.0\r\nConnection: keep-alive\r\n\r\n";
        if let Ok(_) = parser.parse_yet(request_str.as_bytes(), &parse_settings) {
            let request = &parser.request;
            assert_eq!(request.method(), "POST");
            assert_eq!(request.path(), "/index");
            assert_eq!(request.raw_query(), b"a=1&b=2;c=3");
            assert_eq!(request.version, HttpVersion::Http1_0);
            assert!(!request.headers.is_empty());
        } else {
            assert!(false);
        }

        parser.restart();

        let request_str = "POST / HTTP/1.0\r\nConnection: keep-alive\r\nTest: some\r\n\r\n";
        if let Ok(_) = parser.parse_yet(request_str.as_bytes(), &parse_settings) {
            let request = &parser.request;
            assert_eq!(
                request.headers,
                vec![
                    Header {
                        name: "Connection".to_string(),
                        value: "keep-alive".to_string()
                    },
                    Header { name: "Test".to_string(), value: "some".to_string() }
                ]
            );
        } else {
            assert!(false);
        }

        parser.restart();

        let request_str = "";
        if parser.parse_yet(request_str.as_bytes(), &parse_settings).is_ok() {
            assert!(false);
        }

        parser.restart();

        let request_str = "/index?a=1&b=2;c=3 HTTP/1.0\r\nConnection: keep-alive\r\n\r\n";
        if parser.parse_yet(request_str.as_bytes(), &parse_settings).is_ok() {
            assert!(false);
        }

        parser.restart();

        let request_str = "GET /ws /index?a=1&b=2;c=3 HTTP/1.0\r\nConnection: keep-alive\r\n\r\n";
        if Parser::new().parse_yet(request_str.as_bytes(), &parse_settings).is_ok() {
            assert!(false);
        }

        parser.restart();

        // usupported protocol
        let request_str = "GET / HTTP/1.5\r\n\r\n";
        match Parser::new().parse_yet(request_str.as_bytes(), &parse_settings) {
            Ok(_) => {
                assert!(false);
            }
            Err(err) => {
                if let RequestError::UnsupportedProtocol = err {
                } else {
                    assert!(false);
                }
            }
        }

        let request_str = "GET / HTTP/1.1 \r\nConnection: keep-alive\r\n";
        if Parser::new().parse_yet(request_str.as_bytes(), &parse_settings).is_ok() {
            assert!(false);
        }

        let request_str = "GET / HTTP/1.1\r\n: sd\r\n\r\n";
        if Parser::new().parse_yet(request_str.as_bytes(), &parse_settings).is_ok() {
            assert!(false);
        }

        let request_str = "GET / HTTP/1.1\r\n : sd\r\n\r\n";
        assert!(Parser::new().parse_yet(request_str.as_bytes(), &parse_settings).is_ok());

        let request_str = "GET / HTTP/1.1\r\nSD:\r\n\r\n";
        if Parser::new().parse_yet(request_str.as_bytes(), &parse_settings).is_ok() {
            assert!(false);
        }

        let request_str = "GET / HTTP/1.1\r\nSD: \r\n\r\n";
        if Parser::new().parse_yet(request_str.as_bytes(), &parse_settings).is_ok() {
            assert!(false);
        }
    }

    #[test]
    fn limits() {
        let parse_settings = ParseHttpRequestSettings {
            method_len_limit: 5,
            path_len_limit: 512,
            query_len_limit: 512,
            headers_count_limit: 2,
            header_name_len_limit: 5,
            header_value_len_limit: 8,
            pipelining_requests_limit: 12,
        };

        // norm
        let request_str = "GET / HTTP/1.1\r\n1234: abc\r\n\r\n";
        if let Err(_) = Parser::new().parse_yet(request_str.as_bytes(), &parse_settings) {
            assert!(false);
        }

        let request_str = "GET / HTTP/1.1\r\n12345: abc\r\n\r\n";
        if let Err(_) = Parser::new().parse_yet(request_str.as_bytes(), &parse_settings) {
            assert!(false);
        }

        let request_str = "GET / HTTP/1.1\r\n123456: abc\r\n\r\n";
        if let Ok(_) = Parser::new().parse_yet(request_str.as_bytes(), &parse_settings) {
            assert!(false);
        }

        // headers count limit--------------------------------------------
        // less
        let request_str = "GET / HTTP/1.1\r\nabcd: as\r\n\r\n";
        if let Err(_) = Parser::new().parse_yet(request_str.as_bytes(), &parse_settings) {
            assert!(false);
        }

        // equal
        let request_str = "GET / HTTP/1.1\r\nabcd: as\r\nAAA: 12\r\n\r\n";
        if let Err(_) = Parser::new().parse_yet(request_str.as_bytes(), &parse_settings) {
            assert!(false);
        }

        // more
        let request_str = "GET / HTTP/1.1\r\nabcd: as\r\nAAA: 12\r\nVBWER: ASD2\r\n\r\n";
        if let Ok(_) = Parser::new().parse_yet(request_str.as_bytes(), &parse_settings) {
            assert!(false);
        }

        // header value limit--------------------------------------------
        // less
        let request_str = "GET / HTTP/1.1\r\nabcd: as\r\n\r\n";
        if let Err(err) = Parser::new().parse_yet(request_str.as_bytes(), &parse_settings) {
            if let RequestError::HeaderValueLenLimit = err {
                assert!(false);
            }
        }

        // equal
        let request_str = "GET / HTTP/1.1\r\nxyz: bcafghs\r\n\r\n";
        if let Err(_) = Parser::new().parse_yet(request_str.as_bytes(), &parse_settings) {
            assert!(false);
        }

        // more
        let request_str = "GET / HTTP/1.1\r\nxyz: bcaajsxs\r\n\r\n";
        if let Ok(_) = Parser::new().parse_yet(request_str.as_bytes(), &parse_settings) {
            assert!(false);
        }

        // empty header---------------------------------------------------
        let request_str = "GET / HTTP/1.1\r\n: abcasdf\r\n\r\n";
        if let Err(err) = Parser::new().parse_yet(request_str.as_bytes(), &parse_settings) {
            if let RequestError::EmptyHeaderName = err {
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }
    }
}
