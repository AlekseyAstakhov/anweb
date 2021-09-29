use crate::request::{RequestData, HttpVersion, ConnectionType};
use crate::response::{HTTP_CODES_WITH_NAME_BY_CODE, http_status_code_with_name, need_close_by_request};

#[test]
fn close_by_request() {
    let mut request = RequestData::new();
    request.version = HttpVersion::Http1_0;
    request.connection_type = Some(ConnectionType::Close);
    assert_eq!(need_close_by_request(&request), true);

    request.version = HttpVersion::Http1_0;
    request.connection_type = Some(ConnectionType::KeepAlive);
    assert_eq!(need_close_by_request(&request), false);

    // by default in HTTP/1.0 connection close
    request.version = HttpVersion::Http1_0;
    request.connection_type = None;
    assert_eq!(need_close_by_request(&request), true);

    request.version = HttpVersion::Http1_1;
    request.connection_type = Some(ConnectionType::Close);
    assert_eq!(need_close_by_request(&request), true);

    request.version = HttpVersion::Http1_1;
    request.connection_type = Some(ConnectionType::KeepAlive);
    assert_eq!(need_close_by_request(&request), false);

    // by default in HTTP/1.1 connection keep-alive
    request.version = HttpVersion::Http1_1;
    request.connection_type = None;
    assert_eq!(need_close_by_request(&request), false);
}

#[test]
fn http_code_name_test() {
    for t in HTTP_CODES_WITH_NAME_BY_CODE {
        assert_eq!(http_status_code_with_name(t.0), t.1);
    }
}
