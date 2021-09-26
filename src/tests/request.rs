#[cfg(test)]
use crate::request::{Header, HttpVersion, RequestError};
use crate::request_parser::{ParseHttpRequestSettings, Parser};

impl PartialEq for Header {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.value == other.value
    }
}

#[test]
fn parse() {
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
    if let Ok((_request, surplus)) = parser.parse_yet(request_str.as_bytes(), &parse_settings) {
        assert!(surplus.is_empty());
    } else {
        assert!(false);
    }

    let mut parser = Parser::new();

    let request_str = "GET / HTTP/1.1\r\nConnection: keep-alive\r\n\r\naaa";
    if let Ok((_request, surplus)) = parser.parse_yet(request_str.as_bytes(), &parse_settings) {
        assert!(surplus.len() == 3);
    } else {
        assert!(false);
    }

    let mut parser = Parser::new();

    let request_str = "GET /index HTTP/1.1\r\n\r\n";
    if let Ok((request, _)) = parser.parse_yet(request_str.as_bytes(), &parse_settings) {
        assert_eq!(request.method(), "GET");
        assert_eq!(request.path(), "/index");
        assert_eq!(request.raw_query(), b"");
        assert_eq!(request.version, HttpVersion::Http1_1);
        assert!(request.headers.is_empty());
    } else {
        assert!(false);
    }

    let mut parser = Parser::new();

    let request_str = "POST /index?a=1&b=2;c=3 HTTP/1.0\r\nConnection: keep-alive\r\n\r\n";
    if let Ok((request, _)) = parser.parse_yet(request_str.as_bytes(), &parse_settings) {
        assert_eq!(request.method(), "POST");
        assert_eq!(request.path(), "/index");
        assert_eq!(request.raw_query(), b"a=1&b=2;c=3");
        assert_eq!(request.version, HttpVersion::Http1_0);
        assert!(!request.headers.is_empty());
    } else {
        assert!(false);
    }

    let mut parser = Parser::new();

    let request_str = "POST / HTTP/1.0\r\nConnection: keep-alive\r\nTest: some\r\n\r\n";
    if let Ok((request, _)) = parser.parse_yet(request_str.as_bytes(), &parse_settings) {
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

    let mut parser = Parser::new();

    let request_str = "";
    if parser.parse_yet(request_str.as_bytes(), &parse_settings).is_ok() {
        assert!(false);
    }

    let mut parser = Parser::new();

    let request_str = "/index?a=1&b=2;c=3 HTTP/1.0\r\nConnection: keep-alive\r\n\r\n";
    if parser.parse_yet(request_str.as_bytes(), &parse_settings).is_ok() {
        assert!(false);
    }

    let request_str = "GET /ws /index?a=1&b=2;c=3 HTTP/1.0\r\nConnection: keep-alive\r\n\r\n";
    if Parser::new().parse_yet(request_str.as_bytes(), &parse_settings).is_ok() {
        assert!(false);
    }

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
