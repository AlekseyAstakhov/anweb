use crate::query::{parse_query, QueryNameValue};
use crate::tests::request::test_request;
use crate::request::HttpVersion;

impl<'a> PartialEq for QueryNameValue<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.value == other.value
    }
}

#[test]
fn parse() {
    assert!(parse_query(b"").parts.is_empty());
    assert!(parse_query(b"&").parts.is_empty());
    assert!(parse_query(b"&&").parts.is_empty());
    assert!(!parse_query(b"x").parts.is_empty());
    assert_eq!(parse_query(b"x=").parts, vec![QueryNameValue { name: b"x", value: b"" }]);
    assert_eq!(parse_query(b"x=1").parts, vec![QueryNameValue { name: b"x", value: b"1" }]);
    assert_eq!(parse_query(b"x&").parts, vec![QueryNameValue { name: b"x", value: b"" }]);
    assert_eq!(parse_query(b"x&y").parts, vec![QueryNameValue { name: b"x", value: b"" }, QueryNameValue { name: b"y", value: b"" }]);
    assert_eq!(parse_query(b"x=1&y=").parts, vec![QueryNameValue { name: b"x", value: b"1" }, QueryNameValue { name: b"y", value: b"" }]);
    assert_eq!(parse_query(b"x=1&y=1").parts, vec![QueryNameValue { name: b"x", value: b"1" }, QueryNameValue { name: b"y", value: b"1" }]);
    assert_eq!(parse_query(b"x=1&y=1").parts, vec![QueryNameValue { name: b"x", value: b"1" }, QueryNameValue { name: b"y", value: b"1" }]);
    assert_eq!(parse_query(b"x&y;z").parts, vec![QueryNameValue { name: b"x", value: b"" }, QueryNameValue { name: b"y", value: b"" }, QueryNameValue { name: b"z", value: b"" }]);
    assert_eq!(
        parse_query(b"abc=xyz&test=check&xyz=abc").parts,
        vec![QueryNameValue { name: b"abc", value: b"xyz" }, QueryNameValue { name: b"test", value: b"check" }, QueryNameValue { name: b"xyz", value: b"abc" }]
    );
}

#[test]
pub fn local_host() {
    test_request(
        9091,
        "GET /query?first=text1&second=utf-8+%E0%AC%B6%E1%A8%87%D8%86 HTTP/1.0\r\n\
                    Host: 127.0.0.1:8080\r\n\
                    Cache-Control: max-age=0\r\n\
                    sec-ch-ua: \"Chromium\";v=\"94\", \"Google Chrome\";v=\"94\", \";Not A Brand\";v=\"99\"\r\n\
                    sec-ch-ua-mobile: ?0\r\n\
                    sec-ch-ua-platform: \"Linux\"\r\n\
                    Upgrade-Insecure-Requests: 1\r\n\
                    User-Agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/94.0.4606.54 Safari/537.36\r\nAccept: text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.9\r\nSec-Fetch-Site: same-origin\r\nSec-Fetch-Mode: navigate\r\nSec-Fetch-User: ?1\r\nSec-Fetch-Dest: document\r\nReferer: http://127.0.0.1:8080/\r\nAccept-Encoding: gzip, deflate, br\r\nAccept-Language: ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7\r\n\r\n",
        |request| {
            assert_eq!(request.method(), "GET");
            assert_eq!(request.path(), "/query");
            assert_eq!(request.version(), &HttpVersion::Http1_0);
            let query = request.query();
            assert_eq!(query.value("first"), Some("text1".to_string()));
            assert_eq!(query.value_at(1), Some("utf-8 ଶᨇ؆".to_string()));

            request.response(200).send();
        },
        |response| {
            let response_str = std::str::from_utf8(response);
            assert!(response_str.is_ok());

            if let Ok(response) = response_str {
                assert_eq!(
                    &response[..23],
                    "HTTP/1.0 200 OK\r\n\
                    Date: "
                );
                assert_eq!(
                    &response[52..],
                    "\r\n\
                    Content-Length: 0\r\n\r\n"
                );
            }
        }
    );
}
