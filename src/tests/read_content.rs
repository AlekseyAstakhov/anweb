use crate::tests::request::test_request;
use crate::request::HttpVersion;
use std::sync::Arc;

#[test]
fn empty() {
    // with 0 in "Content-Length" header
    test_request(
        9094,
        b"POST / HTTP/1.1\r\n\
                    Content-Length: 0\r\n\
                    \r\n",
        |request| {
            assert_eq!(request.method(), "POST");
            assert_eq!(request.path(), "/");
            assert_eq!(request.version(), &HttpVersion::Http1_1);

            let mut content = vec![];
            request.read_content(move |data, complete| {
                content.extend_from_slice(data);
                if let Some(request) = complete {
                    assert_eq!(&content, b"");
                    request.response(200).close().send();
                }
                Ok(())
            })
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
                Content-Length: 0\r\n\
                \r\n"
            );
        }
    );

    // without "Content-Length" header
    test_request(
        9094,
        b"POST / HTTP/1.1\r\n\r\n",
        |request| {
            assert_eq!(request.method(), "POST");
            assert_eq!(request.path(), "/");
            assert_eq!(request.version(), &HttpVersion::Http1_1);

            let mut content = vec![];
            request.read_content(move |data, complete| {
                content.extend_from_slice(data);
                if let Some(request) = complete {
                    assert_eq!(&content, b"");
                    request.response(200).close().send();
                }
                Ok(())
            })
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
                Content-Length: 0\r\n\
                \r\n"
            );
        }
    );
}

#[test]
fn small_content() {
    test_request(
        9095,
        b"POST / HTTP/1.1\r\n\
                    Content-Type: Content-Type: text/plain; charset=utf-8\r\n\
                    Content-Length: 12\r\n\
                    \r\n\
                    Hello world!",
        |request| {
            assert_eq!(request.method(), "POST");
            assert_eq!(request.path(), "/");
            assert_eq!(request.version(), &HttpVersion::Http1_1);

            let mut content = vec![];
            request.read_content(move |data, complete| {
                content.extend_from_slice(data);
                if let Some(request) = complete {
                    assert_eq!(&content, b"Hello world!");
                    request.response(200).close().send();
                }
                Ok(())
            })
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
                Content-Length: 0\r\n\
                \r\n"
            );
        }
    );
}

#[test]
fn big_content() {
    dbg!();

    const LEN: usize = 10000000;
    let mut origin_content = Vec::with_capacity(LEN);
    for i in 0..LEN {
        origin_content.push(i as u8);
    }

    let origin_content = Arc::new(origin_content);

    let mut request = vec![];
    request.extend_from_slice(
        b"POST / HTTP/1.1\r\n\
        Content-Type: application/octet-stream\r\n\
        Content-Length: "
    );

    request.extend_from_slice(LEN.to_string().as_bytes());
    request.extend_from_slice(b"\r\n\r\n");
    request.extend_from_slice(&origin_content);

    test_request(
        9096,
        &request,
        move |request| {
            assert_eq!(request.method(), "POST");
            assert_eq!(request.path(), "/");
            assert_eq!(request.version(), &HttpVersion::Http1_1);

            let origin_content = origin_content.clone();
            let mut content = vec![];
            request.read_content(move |data, complete| {
                content.extend_from_slice(data);
                if let Some(request) = complete {
                    let received_contant_is_same_original = &content[..] == &origin_content[..];
                    assert!(received_contant_is_same_original);
                    request.response(200).close().send();
                }
                Ok(())
            })
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
                Content-Length: 0\r\n\
                \r\n"
            );
        }
    );
}

