use crate::request::{ConnectionType, HttpVersion, Request};
use crate::http_session::HttpSession;

pub struct Response<'a, 'b, 'c, 'd, 'e> {
    code: u16,
    content: &'a[u8],
    content_type: &'b str,
    http_session: &'c HttpSession,
    /// If Some - Connection header will be set from value.
    /// If None - Connection header will be set by request Connection header and HTTP version.
    keep_alive_connection: Option<bool>,
    /// Extra headers.
    headers: Option<&'e str>,
    cookies: Option<&'d str>,
}

impl<'a, 'b, 'c, 'd, 'e> Response<'a, 'b, 'c, 'd, 'e> {
    pub fn send(&self, request: &Request) {
        let mut response = Vec::from(format!(
            "{} {}\r\n\
         Date: {}\r\n\
         {}\
         Content-Length: {}\r\n\
         Content-Type: {}\r\n\
         {}\
         {}\
         \r\n",
            request.version.to_string_for_response(),
            http_status_code_with_name(self.code),
            self.http_session.http_date_string(),
            self.connection_str(&request),
            self.content.len(),
            self.content_type,
            if let Some(headers) = self.headers { headers } else { "" },
            if let Some(cookies) = self.cookies { cookies } else { "" },
        ));

        response.extend_from_slice(self.content);

        self.http_session.response_raw(&response);
    }

    #[inline(always)]
    pub fn text(&mut self, text: &'a str) -> &mut Self {
        self.content_type = "text/plain; charset=utf-8";
        self.content = text.as_bytes();
        self
    }

    #[inline(always)]
    pub fn html(&mut self, html: &'a str) -> &mut Self {
        self.content_type = "text/html; charset=utf-8";
        self.content = html.as_bytes();
        self
    }

    #[inline(always)]
    pub fn wasm(&mut self, wasm_data: &'a [u8]) -> &mut Self {
        self.content_type = "application/wasm";
        self.content = wasm_data;
        self
    }

    /// By default connection header set by connection header and http version of request.
    /// If call this function the connection header (keep_alive/close) will be set from this value.
    #[inline(always)]
    pub fn keep_alive(&mut self, keep_alive: bool) -> &mut Self {
        self.keep_alive_connection = Some(keep_alive);
        self
    }

    /// Set extra headers.
    /// Note: must not contain headers "Date", "Content-Length" and "Content-Type" because
    /// they will be set automatically when building the response.
    #[inline(always)]
    pub fn headers(&mut self, headers: &'e str) -> &mut Self {
        self.headers = Some(headers);
        self
    }

    #[inline(always)]
    pub fn cookies(&mut self, cookies: &'d str) -> &mut Self {
        self.cookies = Some(cookies);
        self
    }

    pub(crate) fn new(code: u16, http_session: &'c HttpSession) -> Self {
        Response {
            code,
            content: b"",
            http_session,
            content_type: "",
            keep_alive_connection: None,
            headers: None,
            cookies: None,
        }
    }

    fn connection_str<'r>(&self, request: &'r Request) -> &'static str {
        if let Some(keep_alive_connection) = self.keep_alive_connection {
            if keep_alive_connection {
                "Connection: keep_alive\r\n"
            } else {
                "Connection: close\r\n"
            }
        } else {
            connection_str_by_request(request)
        }
    }
}

pub fn connection_str_by_request(request: &Request) -> &'static str {
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

/// Return code name by code number.
pub fn http_status_code_with_name(code: u16) -> &'static str {
    match HTTP_CODES_WITH_NAME_BY_CODE.binary_search_by(|probe| probe.0.cmp(&code)) {
        Ok(index) => HTTP_CODES_WITH_NAME_BY_CODE[index].1,
        Err(_) => "",
    }
}

// from https://en.wikipedia.org/wiki/List_of_HTTP_status_codes
pub static HTTP_CODES_WITH_NAME_BY_CODE: &[(u16, &str)] = &[
    // The server has received the request headers and the client should proceed to send the request
    // body (in the case of a request for which a body needs to be sent; for example, a POST request).
    // Sending a large request body to a server after a request has been rejected for inappropriate
    // headers would be inefficient. To have a server check the request's headers, a client
    // must send Expect: 100-continue as a header in its initial request and receive a 100 Continue
    // status code in response before sending the body. If the client receives an error code
    // such as 403 (Forbidden) or 405 (Method Not Allowed) then it should not send the request's body.
    // The response 417 Expectation Failed indicates that the request should be repeated without
    // the Expect header as it indicates that the server does not support expectations
    // (this is the case, for example, of HTTP/1.0 servers)
    (100, "100 Continue"),
    // The requester has asked the server to switch protocols and the server has agreed to do so.
    (101, "101 Switching Protocols"),
    // A WebDAV request may contain many sub-requests involving file operations, requiring a long time
    // to complete the request. This code indicates that the server has received and is processing
    // the request, but no response is available yet.[6] This prevents the client from timing out
    // and assuming the request was lost.
    (102, "102 Processing"),
    // Used to return some response headers before final HTTP message.
    (103, "103 Early Hints"),

    // Standard response for successful HTTP requests. The actual response will depend on the request
    // method used. In a GET request, the response will contain an entity corresponding to
    // the requested resource. In a POST request, the response will contain an entity describing
    // or containing the result of the action.[8]
    (200, "200 OK"),
    // The request has been fulfilled, resulting in the creation of a new resource.
    (201, "201 Created"),
    // The request has been accepted for processing, but the processing has not been completed.
    // The request might or might not be eventually acted upon,
    // and may be disallowed when processing occurs.[10]
    (202, "202 Accepted"),
    // The server is a transforming proxy (e.g. a Web accelerator)
    // that received a 200 OK from its origin, but is returning a modified
    // version of the origin's response.
    (203, "203 Non-Authoritative Information"),
    // The server successfully processed the request, and is not returning any content.
    (204, "204 No Content"),
    // The server successfully processed the request, asks that the requester reset its document view,
    // and is not returning any content.
    (205, "205 Reset Content"),
    // The server is delivering only part of the resource (byte serving) due to a range header
    // sent by the client. The range header is used by HTTP clients to enable resuming of
    // interrupted downloads, or split a download into multiple simultaneous streams.
    (206, "206 Partial Content"),
    // The message body that follows is by default an XML message and can contain a
    // number of separate response codes, depending on how many sub-requests were made.
    (207, "207 Multi-Status"),
    // The members of a DAV binding have already been enumerated in a preceding part
    // of the (multistatus) response, and are not being included again.
    (208, "208 Already Reported"),
    // The server has fulfilled a request for the resource, and the response is a representation
    // of the result of one or more instance-manipulations applied to the current instance.
    (226, "226 IM Used"),

    // Indicates multiple options for the resource from which the client may choose
    // (via agent-driven content negotiation). For example, this code could be used to present
    // multiple video format options, to list files with different filename extensions, or to suggest
    // word-sense disambiguation.
    (300, "300 Multiple Choices"),
    // This and all future requests should be directed to the given URI.[20]
    (301, "301 Moved Permanently"),
    // Tells the client to look at (browse to) another URL. 302 has been superseded by 303 and 307.
    // This is an example of industry practice contradicting the standard.
    // The HTTP/1.0 specification (RFC 1945) required the client to perform a temporary redirect
    // (the original describing phrase was "Moved Temporarily"),[21] but popular browsers implemented
    // 302 with the functionality of a 303 See Other. Therefore, HTTP/1.1 added
    // status codes 303 and 307 to distinguish between the two behaviours.[22] However,
    // some Web applications and frameworks use the 302 status code as if it were the 303.
    (302, "302 Found"),
    // The response to the request can be found under another URI using the GET method.
    // When received in response to a POST (or PUT/DELETE), the client should presume that the server
    // has received the data and should issue a new GET request to the given URI.
    (303, "303 See Other"),
    // Indicates that the resource has not been modified since the version specified by the request
    // headers If-Modified-Since or If-None-Match. In such case, there is no need to retransmit
    // the resource since the client still has a previously-downloaded copy.
    (304, "304 Not Modified"),
    // The requested resource is available only through a proxy, the address for which is provided
    // in the response. For security reasons, many HTTP clients (such as Mozilla Firefox
    // and Internet Explorer) do not obey this status code.
    (305, "305 Use Proxy"),
    // No longer used. Originally meant "Subsequent requests should use the specified proxy.
    (306, "306 Switch Proxy"),
    // In this case, the request should be repeated with another URI; however, future requests
    // should still use the original URI. In contrast to how 302 was historically implemented,
    // the request method is not allowed to be changed when reissuing the original request.
    // For example, a POST request should be repeated using another POST request.
    (307, "307 Temporary Redirect"),
    // The request and all future requests should be repeated using another URI. 307 and 308 parallel
    // the behaviors of 302 and 301, but do not allow the HTTP method to change. So, for example,
    // submitting a form to a permanently redirected resource may continue smoothly.
    (308, "308 Permanent Redirect"),

    // The server cannot or will not process the request due to an apparent client error
    // (e.g., malformed request syntax, size too large, invalid request message framing,
    // or deceptive request routing).
    (400, "400 Bad Request"),
    // Similar to 403 Forbidden, but specifically for use when authentication is required and
    // has failed or has not yet been provided. The response must include a WWW-Authenticate header
    // field containing a challenge applicable to the requested resource. See Basic access
    // authentication and Digest access authentication. 401 semantically means "unauthorised",
    // the user does not have valid authentication credentials for the target resource.
    // Note: Some sites incorrectly issue HTTP 401 when an IP address is banned from the website
    // (usually the website domain) and that specific address is refused permission to access a website.
    (401, "401 Unauthorized"),
    // Reserved for future use. The original intention was that this code might be used as part of some
    // form of digital cash or micropayment scheme, as proposed, for example, by GNU Taler, but that
    // has not yet happened, and this code is not widely used. Google Developers API uses this status
    // if a particular developer has exceeded the daily limit on requests.[35] Sipgate uses this code
    // if an account does not have sufficient funds to start a call.[36] Shopify uses this code when
    // the store has not paid their fees and is temporarily disabled.[37] Stripe uses this code
    // for failed payments where parameters were correct, for example blocked fraudulent payments.
    (402, "402 Payment Required"),
    // The request contained valid data and was understood by the server, but the server is refusing
    // action. This may be due to the user not having the necessary permissions for a resource
    // or needing an account of some sort, or attempting a prohibited action (e.g. creating a duplicate
    // record where only one is allowed). This code is also typically used if the request provided
    // authentication by answering the WWW-Authenticate header field challenge, but the server did not
    // accept that authentication. The request should not be repeated.
    (403, "403 Forbidden"),
    // The requested resource could not be found but may be available in the future. Subsequent requests
    // by the client are permissible.
    (404, "404 Not Found"),
    // A request method is not supported for the requested resource; for example, a GET request on
    // a form that requires data to be presented via POST, or a PUT request on a read-only resource.
    (405, "405 Method Not Allowed"),
    // The requested resource is capable of generating only content not acceptable according to
    // the Accept headers sent in the request.[39] See Content negotiation.
    (406, "406 Not Acceptable"),
    // The client must first authenticate itself with the proxy.[40]
    (407, "407 Proxy Authentication Required"),
    // The server timed out waiting for the request. According to HTTP specifications:
    // "The client did not produce a request within the time that the server was prepared to wait.
    // The client MAY repeat the request without modifications at any later time."
    (408, "408 Request Timeout"),
    // Indicates that the request could not be processed because of conflict in the current state
    // of the resource, such as an edit conflict between multiple simultaneous updates.
    (409, "409 Conflict"),
    // Indicates that the resource requested is no longer available and will not be available again.
    // This should be used when a resource has been intentionally removed and the resource should
    // be purged. Upon receiving a 410 status code, the client should not request the resource
    // in the future. Clients such as search engines should remove the resource from their indices.
    // Most use cases do not require clients and search engines to purge the resource,
    // and a "404 Not Found" may be used instead.
    (410, "410 Gone"),
    // The request did not specify the length of its content, which is required by the requested resource.
    (411, "411 Length Required"),
    // The server does not meet one of the preconditions that the requester put on the
    // request header fields.
    (412, "412 Precondition Failed"),
    // The request is larger than the server is willing or able to process. Previously called
    // "Request Entity Too Large".
    (413, "413 Payload Too Large"),
    // The URI provided was too long for the server to process. Often the result of too much data being
    // encoded as a query-string of a GET request, in which case it should be converted to a POST request.
    // Called "Request-URI Too Long" previously.
    (414, "414 URI Too Long"),
    // The request entity has a media type which the server or resource does not support. For example,
    // the client uploads an image as image/svg+xml, but the server requires that images use
    // a different format.
    (415, "415 Unsupported Media Type"),
    // The client has asked for a portion of the file (byte serving), but the server cannot supply
    // that portion. For example, if the client asked for a part of the file that lies beyond the
    // end of the file.[49] Called "Requested Range Not Satisfiable" previously.
    (416, "416 Range Not Satisfiable"),
    // The server cannot meet the requirements of the Expect request-header field.
    (417, "417 Expectation Failed"),
    // This code was defined in 1998 as one of the traditional IETF April Fools' jokes, in RFC 2324,
    // Hyper Text Coffee Pot Control Protocol, and is not expected to be implemented by actual
    // HTTP servers.
    // The RFC specifies this code should be returned by teapots requested to brew coffee.
    // This HTTP status is used as an Easter egg in some websites,
    // such as Google.com's I'm a teapot easter egg.
    (418, "418 I'm a teapot"),
    // The request was directed at a server that is not able to produce a response
    // (for example because of connection reuse).
    (421, "421 Misdirected Request"),
    // The request was well-formed but was unable to be followed due to semantic errors.
    (422, "422 Unprocessable Entity"),
    // The resource that is being accessed is locked.
    (423, "423 Locked"),
    // The request failed because it depended on another request and that request failed
    // (e.g., a PROPPATCH).
    (424, "424 Failed Dependency"),
    // Indicates that the server is unwilling to risk processing a request that might be replayed.
    (425, "425 Too Early"),
    // The client should switch to a different protocol such as TLS/1.3, given in the Upgrade header field.
    (426, "426 Upgrade Required"),
    // The origin server requires the request to be conditional. Intended to prevent the 'lost update'
    // problem, where a client GETs a resource's state, modifies it, and PUTs it back to the server,
    // when meanwhile a third party has modified the state on the server, leading to a conflict.
    (428, "428 Precondition Required"),
    // The user has sent too many requests in a given amount of time. Intended for use
    // with rate-limiting schemes.
    (429, "429 Too Many Requests"),
    // The server is unwilling to process the request because either an individual header field,
    // or all the header fields collectively, are too large.
    (431, "431 Request Header Fields Too Large"),
    // A server operator has received a legal demand to deny access to a resource or to a set
    // of resources that includes the requested resource.[59] The code 451 was chosen as a reference
    // to the novel Fahrenheit 451 (see the Acknowledgements in the RFC).
    (451, "451 Unavailable For Legal Reasons"),

    // A generic error message, given when an unexpected condition was encountered and no more specific
    // message is suitable.
    (500, "500 Internal Server Error"),
    // The server either does not recognize the request method, or it lacks the ability to fulfil
    // the request. Usually this implies future availability (e.g., a new feature of a web-service API).
    (501, "501 Not Implemented"),
    // The server was acting as a gateway or proxy and received an invalid response from the upstream server.
    (502, "502 Bad Gateway"),
    // The server cannot handle the request (because it is overloaded or down for maintenance). Generally,
    // this is a temporary state.
    (503, "503 Service Unavailable"),
    // The server was acting as a gateway or proxy and did not receive a timely response from the upstream server.
    (504, "504 Gateway Timeout"),
    // The server does not support the HTTP protocol version used in the request.
    (505, "505 HTTP Version Not Supported"),
    // Transparent content negotiation for the request results in a circular reference.
    (506, "506 Variant Also Negotiates"),
    // The server is unable to store the representation needed to complete the request.
    (507, "507 Insufficient Storage"),
    // The server detected an infinite loop while processing the request
    // (sent instead of 208 Already Reported).
    (508, "508 Loop Detected"),
    // Further extensions to the request are required for the server to fulfil it.
    (510, "510 Not Extended"),
    // The client needs to authenticate to gain network access. Intended for use by intercepting proxies
    // used to control access to the network (e.g., "captive portals" used to require agreement
    // to Terms of Service before granting full Internet access via a Wi-Fi hotspot).
    (511, "511 Network Authentication Required"),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn http_code_name_test() {
        for t in HTTP_CODES_WITH_NAME_BY_CODE {
            assert_eq!(http_status_code_with_name(t.0), t.1);
        }
    }
}