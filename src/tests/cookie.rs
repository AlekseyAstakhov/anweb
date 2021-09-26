use crate::cookie::{parse_cookie, CookieOfRequst};

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
