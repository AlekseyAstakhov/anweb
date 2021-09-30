use crate::cookie::{parse_cookie, CookieOfRequst, Cookie};
use crate::tests::request::test_request;
use crate::request::HttpVersion;

impl<'a> PartialEq for CookieOfRequst<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.value == other.value
    }
}

#[test]
fn parse() {
    assert!(parse_cookie("").is_empty());
    assert!(parse_cookie(";").is_empty());
    assert!(parse_cookie(";;").is_empty());
    assert_eq!(parse_cookie("x"), vec![CookieOfRequst { name: "x", value: "" }]);
    assert_eq!(parse_cookie("x=1"), vec![CookieOfRequst { name: "x", value: "1" }]);
    assert_eq!(parse_cookie("x=ab"), vec![CookieOfRequst { name: "x", value: "ab" }]);
    assert_eq!(parse_cookie(";x"), vec![CookieOfRequst { name: "x", value: "" }]);
    assert_eq!(parse_cookie("x;"), vec![CookieOfRequst { name: "x", value: "" }]);
    assert_eq!(parse_cookie(";x;"), vec![CookieOfRequst { name: "x", value: "" }]);
    assert_eq!(parse_cookie(" x"), vec![CookieOfRequst { name: "x", value: "" }]);
    assert_eq!(parse_cookie(" x;"), vec![CookieOfRequst { name: "x", value: "" }]);
    assert_eq!(parse_cookie("x; "), vec![CookieOfRequst { name: "x", value: "" }]);
    assert_eq!(parse_cookie(" x; "), vec![CookieOfRequst { name: "x", value: "" }]);
    assert_eq!(parse_cookie(" x; "), vec![CookieOfRequst { name: "x", value: "" }]);
    assert_eq!(parse_cookie("x="), vec![CookieOfRequst { name: "x", value: "" }]);
    assert!(parse_cookie("=x").is_empty());
    assert!(parse_cookie(" =x").is_empty());
    assert_eq!(parse_cookie(" x=; "), vec![CookieOfRequst { name: "x", value: "" }]);
    assert_eq!(parse_cookie("x  = qq q "), vec![CookieOfRequst { name: "x  ", value: " qq q " }]);
    assert_eq!(parse_cookie("   x  = qq q "), vec![CookieOfRequst { name: "x  ", value: " qq q " }]);
    assert_eq!(parse_cookie("ab"), vec![CookieOfRequst { name: "ab", value: "" }]);
    assert_eq!(parse_cookie(" abc"), vec![CookieOfRequst { name: "abc", value: "" }]);
    assert_eq!(parse_cookie(" abc=xyz"), vec![CookieOfRequst { name: "abc", value: "xyz" }]);
    assert_eq!(parse_cookie(" abc=xyz;xyz=123"), vec![CookieOfRequst { name: "abc", value: "xyz" }, CookieOfRequst { name: "xyz", value: "123" }]);
    assert_eq!(parse_cookie(" abc=xyz; xyz=123"), vec![CookieOfRequst { name: "abc", value: "xyz" }, CookieOfRequst { name: "xyz", value: "123" }]);

    assert!(parse_cookie("=x").is_empty());
}

#[test]
fn local_host() {
    test_request(
        9093,
        b"GET / HTTP/1.1\r\n\
        Cookie: ABCD=-W-e-QSDEe-QSDEF3erw---W-e-Q-SDEF3erwqew-weqf-;key=Hello world!\r\n\
        Connection: keep-alive\r\n\
        Content-Length: 0\r\n\r\n",
        |request| {
            assert_eq!(request.method(), "GET");
            assert_eq!(request.path(), "/");
            assert_eq!(request.version(), &HttpVersion::Http1_1);

            assert_eq!(request.cookies()[0].name, "ABCD");
            assert_eq!(request.cookies()[0].value, "-W-e-QSDEe-QSDEF3erw---W-e-Q-SDEF3erwqew-weqf-");
            assert_eq!(request.cookies()[1].name, "key");
            assert_eq!(request.cookies()[1].value, "Hello world!");

            let cookie1 = Cookie {
                name: "seasddsf",
                value: "13241abc",
                path: None,
                domain: None,
                http_only: true,
                expires: None,
                max_age: None,
                secure: false,
            }.to_string();

            let cookie2 = Cookie {
                name: "test2",
                value: "xyz",
                path: Some("/"),
                domain: Some("domain"),
                http_only: false,
                expires: Some("Wed"),
                max_age: Some(38),
                secure: true,
            }.to_string();

            let cookies = cookie1 + &cookie2;

            request.response(200).cookies(&cookies).close().send();
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
                Set-Cookie: seasddsf=13241abc; HttpOnly\r\n\
                Set-Cookie: test2=xyz; Path=\"/\"; Domain=\"domain\"; Expires=\"Wed\"; Max-Age=38; Secure\r\n\r\n"
            );
        }
    );
}
