use crate::cookie::Cookie;
use crate::request::{ConnectionType, HttpVersion, Request};

pub fn ok_200(data: &[u8], content_type: &str, request: &Request, date_string: &str) -> Vec<u8> {
    let mut result = Vec::from(format!(
        "{} 200 OK\r\n\
         Date: {}\r\n\
         {}\
         Content-Length: {}\r\n\
         Content-Type: {}\r\n\
         \r\n",
        request.version.to_string_for_response(),
        date_string,
        connection_str_by_request(request),
        data.len(),
        content_type
    ));

    result.extend_from_slice(data);
    result
}

pub fn text_200(text: &str, request: &Request, date_string: &str) -> Vec<u8> {
    Vec::from(format!(
        "{} 200 OK\r\n\
         Date: {}\r\n\
         {}\
         Content-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {}",
        request.version.to_string_for_response(),
        date_string,
        connection_str_by_request(request),
        text.len(),
        text
    ))
}

pub fn html_200(body: &str, request: &Request, date_string: &str) -> Vec<u8> {
    Vec::from(format!(
        "{} 200 OK\r\n\
         Date: {}\r\n\
         {}\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {}",
        request.version.to_string_for_response(),
        date_string,
        connection_str_by_request(request),
        body.len(),
        body
    ))
}

pub fn wasm_200(data: &[u8], request: &Request, date_string: &str) -> Vec<u8> {
    ok_200(data, "application/wasm", request, date_string)
}

pub fn html_with_cookie_200(body: &str, cookie: &Cookie, request: &Request, date_string: &str) -> Vec<u8> {
    Vec::from(format!(
        "{} 200 OK\r\n\
         Date: {}\r\n\
         {}\
         Content-Type: text/html; charset=utf-8\r\n\
         Set-Cookie: {}={}{}{}{}{}{}{}\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {}",
        request.version.to_string_for_response(),
        date_string,
        connection_str_by_request(request),
        cookie.name,
        cookie.value,
        cookie_path_str(cookie.path),
        cookie_domain_str(cookie.domain),
        cookie_expires_str(cookie.expires),
        cookie_max_age_str(cookie.max_age),
        if cookie.secure { "; Secure" } else { "" },
        if cookie.http_only { "; HttpOnly" } else { "" },
        body.len(),
        body
    ))
}

pub fn redirect_303_close(path: &str, request: &Request) -> Vec<u8> {
    Vec::from(format!(
        "{} 303 See Other\r\n\
         Location: {}\r\n\
         Connection: close\r\n\
         Content-Length: 0\r\n\
         \r\n",
        request.version.to_string_for_response(),
        path
    ))
}

pub fn redirect_303_with_cookie(path: &str, cookie: &Cookie, request: &Request, date_string: &str) -> Vec<u8> {
    Vec::from(format!(
        "{} 303 See Other\r\n\
         Date: {}\r\n\
         {}\
         Location: {}\r\n\
         Set-Cookie: {}={}{}{}{}{}{}{}\r\n\
         Content-Length: 0\r\n\
         \r\n",
        request.version.to_string_for_response(),
        date_string,
        connection_str_by_request(request),
        path,
        cookie.name,
        cookie.value,
        cookie_path_str(cookie.path),
        cookie_domain_str(cookie.domain),
        cookie_expires_str(cookie.expires),
        cookie_max_age_str(cookie.max_age),
        if cookie.secure { "; Secure" } else { "" },
        if cookie.http_only { "; HttpOnly" } else { "" }
    ))
}

pub fn empty_bad_request_400(request: &Request, date_string: &str) -> Vec<u8> {
    Vec::from(format!(
        "{} 400 Bad Request\r\n\
         Date: {}\r\n\
         Connection: close\r\n\
         Content-Length: 0\r\n\
         \r\n",
        request.version.to_string_for_response(),
        date_string
    ))
}

pub fn bad_request_with_text_400(text: &str, request: &Request, date_string: &str) -> Vec<u8> {
    Vec::from(format!(
        "{} 400 Bad Request\r\n\
         Date: {}\r\n\
         Connection: close\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {}",
        request.version.to_string_for_response(),
        date_string,
        text.len(),
        text
    ))
}

pub fn unprocessable_entity_empty_422(request: &Request, date_string: &str) -> Vec<u8> {
    Vec::from(format!(
        "{} 422 Unprocessable Entity\r\n\
         Date: {}\r\n\
         {}\
         Content-Length: 0\r\n\
         \r\n",
        request.version.to_string_for_response(),
        date_string,
        connection_str_by_request(request)
    ))
}

pub fn unprocessable_entity_with_text_422(text: &str, request: &Request, date_string: &str) -> Vec<u8> {
    Vec::from(format!(
        "{} 422 Unprocessable Entity\r\n\
         Date: {}\r\n\
         {}\
         Content-Length: {}\r\n\
         \r\n\
         {}",
        request.version.to_string_for_response(),
        date_string,
        connection_str_by_request(request),
        text.len(),
        text
    ))
}

pub fn empty_404(request: &Request, date_string: &str) -> Vec<u8> {
    Vec::from(format!(
        "{} 404 Not Found\r\n\
         Date: {}\r\n\
         {}\
         Content-Length: 0\r\n\
         \r\n",
        request.version.to_string_for_response(),
        date_string,
        connection_str_by_request(request)
    ))
}

pub fn text_404(text: &str, request: &Request, date_string: &str) -> Vec<u8> {
    Vec::from(format!(
        "{} 404 Not Found\r\n\
         Date: {}\r\n\
         {}\
         Content-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {}",
        request.version.to_string_for_response(),
        date_string,
        connection_str_by_request(request),
        text.len(),
        text
    ))
}

pub fn html_404(html: &str, request: &Request, date_string: &str) -> Vec<u8> {
    Vec::from(format!(
        "{} 404 Not Found\r\n\
         Date: {}\r\n\
         {}\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {}",
        request.version.to_string_for_response(),
        date_string,
        connection_str_by_request(request),
        html.len(),
        html
    ))
}

pub fn empty_500(request: &Request, date_string: &str) -> Vec<u8> {
    Vec::from(format!(
        "{} 500 Internal Server Error\r\n\
         Date: {}\r\n\
         {}\
         Content-Length: 0\r\n\
         \r\n",
        request.version.to_string_for_response(),
        date_string,
        connection_str_by_request(request)
    ))
}

pub fn text_500(text: &str, request: &Request, date_string: &str) -> Vec<u8> {
    Vec::from(format!(
        "{} 500 Internal Server Error\r\n\
         Date: {}\r\n\
         {}\
         Content-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {}",
        request.version.to_string_for_response(),
        date_string,
        connection_str_by_request(request),
        text.len(),
        text
    ))
}

pub fn html_500(html: &str, request: &Request, date_string: &str) -> Vec<u8> {
    Vec::from(format!(
        "{} 500 Internal Server Error\r\n\
         Date: {}\r\n\
         {}\
         Content-Type: text/html; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {}",
        request.version.to_string_for_response(),
        date_string,
        connection_str_by_request(request),
        html.len(),
        html
    ))
}

pub fn connection_str_by_request(request: &Request) -> &str {
    if let Some(connection_type) = &request.connection_type {
        match connection_type {
            ConnectionType::KeepAlive => "Connection: keep_alive\r\n",
            _ => "Connection: close\r\n",
        }
    } else {
        match request.version {
            HttpVersion::Http1_1 => "Connection: keep_alive\r\n",
            _ => "",
        }
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
