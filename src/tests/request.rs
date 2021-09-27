#[cfg(test)]
use crate::request::{Header, HttpVersion, RequestError};
use crate::request_parser::{ParseHttpRequestSettings, Parser};
use crate::server::{Event, Server};
use std::thread::sleep;
use std::net::TcpStream;
use std::io::{Write, Read};
use std::time::Duration;
use crate::request::Request;

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
        assert_eq!(surplus.len(), 3);
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

/// Starts the server on localhost, opens the client socket,
/// makes request ('raw_request') to the server,
/// calls callback when request is received on server side, reads response,
/// calls callback when response is received, and stops the server.
pub fn test_request(port: u16, raw_request: &str, on_request: impl FnMut(Request)  + Send + Clone + 'static, on_response: impl FnMut(&[u8]) + Send + Clone + 'static) {
    let server = Server::new(&([0, 0, 0, 0], port).into());
    assert!(server.is_ok());
    if let Ok(server) = server {
        let num_threads = server.num_threads;
        let stopper = server.stopper();
        let raw_request = raw_request.to_string();
        let server_run_res = server.run(move |server_event| {
            match server_event {
                Event::Incoming(tcp_session) => {
                    let mut on_request = on_request.clone();
                    tcp_session.to_http(move |request| {
                        assert!(request.is_ok());
                        on_request(request?);
                        Ok(())
                    });
                }
                Event::Started => {
                    let stopper = stopper.clone();
                    let mut on_response = on_response.clone();
                    let raw_request = raw_request.clone();
                    std::thread::spawn(move || {
                        let addr = &format!("127.0.0.1:{}", port.to_string());
                        let tcp_stream = TcpStream::connect(addr);
                        assert!(tcp_stream.is_ok());
                        if let Ok(mut tcp_stream) = tcp_stream {
                            let res = tcp_stream.set_write_timeout(Some(Duration::from_millis(100)));
                            assert!(res.is_ok());
                            let res = tcp_stream.write(raw_request.as_bytes());
                            assert!(res.is_ok());

                            let mut response: Vec<u8> = Vec::new();
                            let res = tcp_stream.set_read_timeout(Some(Duration::from_millis(300)));
                            assert!(res.is_ok());
                            let res = tcp_stream.read_to_end(&mut response);
                            assert!(res.is_ok());
                            on_response(&response);

                            stopper.stop();
                            for _ in 0..num_threads {
                                if let Ok(_) = TcpStream::connect(addr) {
                                    sleep(Duration::from_millis(30));
                                }
                            }
                        }
                    });
                }
                _ => {}
            }
        });
        assert!(server_run_res.is_ok());
    }
}

#[test]
fn hello_world() {
    test_request(
        9090,
        "GET / HTTP/1.1\r\n\
                    Host: 127.0.0.1:8080\r\n\
                    User-Agent: Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:92.0) Gecko/20100101 Firefox/92.0\r\n\
                    Accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8\r\n\
                    Accept-Language: ru-RU,ru;q=0.8,en-US;q=0.5,en;q=0.3\r\n\
                    Accept-Encoding: gzip, deflate\r\n\
                    Connection: keep-alive\r\n\
                    Upgrade-Insecure-Requests: 1\r\n\
                    Sec-Fetch-Dest: document\r\n\
                    Sec-Fetch-Mode: navigate\r\n\
                    Sec-Fetch-Site: none\r\n\
                    Sec-Fetch-User: ?1\r\n\
                    Cache-Control: max-age=0\r\n\r\n",
        |request| {
            assert_eq!(request.method(), "GET");
            assert_eq!(request.path(), "/");
            assert_eq!(request.version(), &HttpVersion::Http1_1);
            request.response(200).close().text("Hello world!").send();
        },
        |response| {
            assert_eq!(
                &response[..23],
                b"HTTP/1.1 200 OK\r\n\
                Date: "
            );
            assert_eq!(
                &response[52..],
                b"\r\n\
                Connection: close\r\n\
                Content-Length: 12\r\n\
                Content-Type: text/plain; charset=utf-8\r\n\r\n\
                Hello world!"
            );
        }
    );
}
