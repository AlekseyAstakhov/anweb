use crate::request::HttpVersion;
use crate::tests::request::test_request;
use crate::query::parse_query;

#[test]
fn localhost() {
    test_request(
        9092,
        "POST /form HTTP/1.1\r\n\
        Host: 127.0.0.1:8080\r\n\
        Connection: close\r\n\
        Content-Length: 70\r\n\
        Cache-Control: max-age=0\r\n\
        sec-ch-ua: \" Not;A Brand\";v=\"99\", \"Opera\";v=\"79\", \"Chromium\";v=\"93\"\r\n\
        sec-ch-ua-mobile: ?0\r\nsec-ch-ua-platform: \"Linux\"\r\n\
        Upgrade-Insecure-Requests: 1\r\n\
        Origin: http://127.0.0.1:8080\r\n\
        Content-Type: application/x-www-form-urlencoded\r\n\
        User-Agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/93.0.4577.82 Safari/537.36 OPR/79.0.4143.50\r\n\
        Accept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.9\r\n\
        Sec-Fetch-Site: same-origin\r\n\
        Sec-Fetch-Mode: navigate\r\n\
        Sec-Fetch-User: ?1\r\n\
        Sec-Fetch-Dest: document\r\n\
        Referer: http://127.0.0.1:8080/\r\n\
        Accept-Encoding: gzip, deflate, br\r\n\
        Accept-Language: en-US,en;q=0.9\r\n\r\n\
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
