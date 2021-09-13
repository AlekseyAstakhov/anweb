/// Cookie that the server sends to the client.
#[derive(Debug)]
pub struct Cookie<'a> {
    /// Cookie name. Can't be empty.
    pub name: &'a str,
    /// Cookie value. Can be empty.
    pub value: &'a str,

    /// Path attribute indicates a URL path that must exist in the requested resource before sending the Cookie header.
    pub path: Option<&'a str>,
    /// Domain attribute specifies those hosts to which the cookie will be sent. If not specified, defaults to the host portion of the current document location (but not including subdomains).
    pub domain: Option<&'a str>,
    /// Expires attribute indicates cookie expiration date.
    pub expires: Option<&'a str>,

    // Max-Age attribute indicates the maximum lifetime of the cookie in seconds.
    pub max_age: Option<i32>,
    /// HttpOnly is an additional flag. Using the HttpOnly flag when generating a cookie helps mitigate the risk of client side script accessing the protected cookie (if the browser supports it).
    pub http_only: bool,
    /// Secure attribute. A secure cookie is only sent to the server with an encrypted request over the HTTPS protocol.
    pub secure: bool,
}

impl<'a> Cookie<'a> {
    /// Prepared cookie for remove on the browser side.
    pub fn remove(name: &'a str) -> Self {
        Cookie {
            name,
            value: "",
            path: None,
            domain: None,
            expires: None,
            max_age: Some(0),
            http_only: true,
            secure: false,
        }
    }
}

/// Cookie that the received from client.
#[derive(Debug)]
pub struct ReceivedCookie<'a> {
    /// Cookie name. Can't be empty.
    pub name: &'a str,
    /// Cookie value. Can be empty.
    pub value: &'a str,
}

/// Cookies that the received from client.
#[derive(Debug)]
pub struct ReceivedCookies<'a> {
    pub cookies: Vec<ReceivedCookie<'a>>,
}

impl<'a> std::ops::Deref for ReceivedCookies<'a> {
    type Target = Vec<ReceivedCookie<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.cookies
    }
}

impl<'a> std::ops::DerefMut for ReceivedCookies<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cookies
    }
}

impl<'a> ReceivedCookies<'a> {
    /// Returns first cookie value by name.
    pub fn value(&self, name: &str) -> Option<&str> {
        for cookie in self.iter() {
            if cookie.name == name {
                return Some(cookie.value);
            }
        }

        None
    }
}

/// Convert cookie string from http header to the struct.
pub fn parse_cookie(cookies_header_value: &str) -> ReceivedCookies {
    let mut result = ReceivedCookies { cookies: Vec::new() };

    let cookies = cookies_header_value.split(|ch| ch == ';');
    for cookie in cookies {
        let begin_idx = cookie.bytes().position(|ch| ch != b' ');
        if let Some(begin_idx) = begin_idx {
            let cookie = &cookie[begin_idx..];
            let assignment_pos = cookie.bytes().position(|ch| ch == b'=');
            if let Some(assignment_pos) = assignment_pos {
                // name found
                if assignment_pos > 0 {
                    let name = &cookie[..assignment_pos];
                    let value = &cookie[assignment_pos + 1..];
                    result.push(ReceivedCookie { name, value })
                }
            } else {
                // only name found "abc" or "abc="
                let name = &cookie[..];
                let value = "";
                result.push(ReceivedCookie { name, value })
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    impl<'a> PartialEq for ReceivedCookie<'a> {
        fn eq(&self, other: &Self) -> bool {
            self.name == other.name && self.value == other.value
        }
    }

    #[test]
    fn test() {
        assert!(parse_cookie("").cookies.is_empty());
        assert!(parse_cookie(";").cookies.is_empty());
        assert!(parse_cookie(";;").cookies.is_empty());
        assert_eq!(parse_cookie("x").cookies, vec![ReceivedCookie { name: "x", value: "" }]);
        assert_eq!(parse_cookie("x=1").cookies, vec![ReceivedCookie { name: "x", value: "1" }]);
        assert_eq!(parse_cookie("x=ab").cookies, vec![ReceivedCookie { name: "x", value: "ab" }]);
        assert_eq!(parse_cookie(";x").cookies, vec![ReceivedCookie { name: "x", value: "" }]);
        assert_eq!(parse_cookie("x;").cookies, vec![ReceivedCookie { name: "x", value: "" }]);
        assert_eq!(parse_cookie(";x;").cookies, vec![ReceivedCookie { name: "x", value: "" }]);
        assert_eq!(parse_cookie(" x").cookies, vec![ReceivedCookie { name: "x", value: "" }]);
        assert_eq!(parse_cookie(" x;").cookies, vec![ReceivedCookie { name: "x", value: "" }]);
        assert_eq!(parse_cookie("x; ").cookies, vec![ReceivedCookie { name: "x", value: "" }]);
        assert_eq!(parse_cookie(" x; ").cookies, vec![ReceivedCookie { name: "x", value: "" }]);
        assert_eq!(parse_cookie(" x; ").cookies, vec![ReceivedCookie { name: "x", value: "" }]);
        assert_eq!(parse_cookie("x=").cookies, vec![ReceivedCookie { name: "x", value: "" }]);
        assert!(parse_cookie("=x").cookies.is_empty());
        assert!(parse_cookie(" =x").cookies.is_empty());
        assert_eq!(parse_cookie(" x=; ").cookies, vec![ReceivedCookie { name: "x", value: "" }]);
        assert_eq!(parse_cookie("x  = qq q ").cookies, vec![ReceivedCookie { name: "x  ", value: " qq q " }]);
        assert_eq!(parse_cookie("   x  = qq q ").cookies, vec![ReceivedCookie { name: "x  ", value: " qq q " }]);
        assert_eq!(parse_cookie("ab").cookies, vec![ReceivedCookie { name: "ab", value: "" }]);
        assert_eq!(parse_cookie(" abc").cookies, vec![ReceivedCookie { name: "abc", value: "" }]);
        assert_eq!(parse_cookie(" abc=xyz").cookies, vec![ReceivedCookie { name: "abc", value: "xyz" }]);
        assert_eq!(parse_cookie(" abc=xyz;xyz=123").cookies, vec![ReceivedCookie { name: "abc", value: "xyz" }, ReceivedCookie { name: "xyz", value: "123" }]);
        assert_eq!(parse_cookie(" abc=xyz; xyz=123").cookies, vec![ReceivedCookie { name: "abc", value: "xyz" }, ReceivedCookie { name: "xyz", value: "123" }]);

        assert!(parse_cookie("=x").cookies.is_empty());
    }
}
