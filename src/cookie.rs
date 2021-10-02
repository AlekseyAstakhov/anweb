/// Cookie that the server sends to the client.
#[derive(Debug)]
pub struct Cookie<'a, 'b, 'c, 'd, 'e> {
    /// Cookie name. Can't be empty.
    pub name: &'a str,
    /// Cookie value. Can be empty.
    pub value: &'b str,

    /// Path attribute indicates a URL path that must exist in the requested resource before sending the Cookie header.
    pub path: Option<&'c str>,
    /// Domain attribute specifies those hosts to which the cookie will be sent. If not specified, defaults to the host portion of the current document location (but not including subdomains).
    pub domain: Option<&'d str>,
    /// Expires attribute indicates cookie expiration date.
    pub expires: Option<&'e str>,

    // Max-Age attribute indicates the maximum lifetime of the cookie in seconds.
    pub max_age: Option<i32>,
    /// HttpOnly is an additional flag. Using the HttpOnly flag when generating a cookie helps mitigate the risk of client side script accessing the protected cookie (if the browser supports it).
    pub http_only: bool,
    /// Secure attribute. A secure cookie is only sent to the server with an encrypted request over the HTTPS protocol.
    pub secure: bool,
}

impl<'a, 'b, 'c, 'd, 'e> Cookie<'a, 'b, 'c, 'd, 'e> {
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

    /// Return string with value prepared for "Set-Cookie" header.
    pub fn header_value(&self) -> String {
        format!("{}={}{}{}{}{}{}{}",
                self.name,
                self.value,
                cookie_path_str(self.path),
                cookie_domain_str(self.domain),
                cookie_expires_str(self.expires),
                cookie_max_age_str(self.max_age),
                if self.secure { "; Secure" } else { "" },
                if self.http_only { "; HttpOnly" } else { "" },
        )
    }
}

impl<'a, 'b, 'c, 'd, 'e> std::fmt::Display for Cookie<'a, 'b, 'c, 'd, 'e> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.write_str(&format!("Set-Cookie: {}={}{}{}{}{}{}{}\r\n",
            self.name,
            self.value,
            cookie_path_str(self.path),
            cookie_domain_str(self.domain),
            cookie_expires_str(self.expires),
            cookie_max_age_str(self.max_age),
            if self.secure { "; Secure" } else { "" },
            if self.http_only { "; HttpOnly" } else { "" },
        ))?;
        Ok(())
    }
}

fn cookie_path_str(path: Option<&str>) -> String {
    if let Some(path) = path {
        return format!("; Path={:?}", path);
    }

    String::new()
}

fn cookie_domain_str(domain: Option<&str>) -> String {
    if let Some(domain) = domain {
        return format!("; Domain={:?}", domain);
    }

    String::new()
}

fn cookie_expires_str(expires: Option<&str>) -> String {
    if let Some(expires) = expires {
        return format!("; Expires={:?}", expires);
    }

    String::new()
}

fn cookie_max_age_str(max_age: Option<i32>) -> String {
    if let Some(max_age) = max_age {
        return format!("; Max-Age={:?}", max_age);
    }

    String::new()
}

/// Cookie that the received from client.
#[derive(Debug)]
pub struct CookieOfRequst<'a> {
    /// Cookie name. Can't be empty.
    pub name: &'a str,
    /// Cookie value. Can be empty.
    pub value: &'a str,
}

/// Convert cookie string from http header to the struct.
pub fn parse_cookie(cookies_header_value: &str) -> Vec<CookieOfRequst> {
    let mut result = Vec::new();

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
                    result.push(CookieOfRequst { name, value })
                }
            } else {
                // only name found "abc" or "abc="
                let name = &cookie[..];
                let value = "";
                result.push(CookieOfRequst { name, value })
            }
        }
    }

    result
}
