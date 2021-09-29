use crate::request::HttpVersion;
use crate::tests::request::test_request;
use crate::query::parse_query;

#[test]
fn localhost() {
    test_request(
        9092,
        "POST /form HTTP/1.1\r\n\
        Connection: close\r\n\
        Content-Type: application/x-www-form-urlencoded\r\n\
        Content-Length: 70\r\n\r\n\
        first=-%E0%A8%8A%E0%B0%88%E0%AF%B5&second=%E0%AF%B5%E0%B0%88%E0%A8%8A-",
        |request| {
            assert_eq!(request.method(), "POST");
            assert_eq!(request.path(), "/form");
            assert_eq!(request.version(), &HttpVersion::Http1_1);
            assert!(request.has_post_form());

            let mut content = vec![];
            request.read_content(move |data, content_is_complite| {
                content.extend_from_slice(data);
                if let Some(request) = content_is_complite {
                    let form = parse_query(&content);
                    assert_eq!(form.value("first"), Some("-ਊఈ௵".to_string()));
                    assert_eq!(form.value("second"), Some("௵ఈਊ-".to_string()));
                    request.response(200).send();
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
                Content-Length: 0\r\n\r\n"
            );
        }
    );
}
