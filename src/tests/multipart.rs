use crate::request::HttpVersion;
use crate::tests::request::test_request;
use crate::multipart::{MultipartParser, MultipartParserEvent};
use std::sync::Arc;
use std::ops::Deref;

#[test]
fn localhost() {

//    let origin_file_data = Arc::new(origin_content);


    let mut content = Vec::from("\
        ---------------573cf973d5228\r\n\
        Content-Disposition: form-data; name=\"field\"\r\n\
        \r\n\
        text\
        ---------------573cf973d5228\r\n\
        Content-Disposition: form-data; name=\"field2\"\r\n\
        \r\n\
        other text\
        ---------------573cf973d5228\r\n\
        Content-Disposition: form-data; name=\"file\"; filename=\"sample.bin\"\r\n\
        Content-Type: application/octet-stream\r\n\
        \r\n");

    const LEN: usize = 10000000;
    let mut origin_file_data = Vec::with_capacity(LEN);
    for i in 0..LEN {
        origin_file_data.push(i as u8);
    }

    content.extend_from_slice(&origin_file_data);

    content.extend_from_slice(b"---------------573cf973d5228--");

    let mut request = Vec::from(format!("\
        POST /form HTTP/1.1\r\n\
        Content-Type: multipart/form-data; boundary=-------------573cf973d5228\r\n\
        Content-Length: {}\r\n\r\n", content.len())
    );

    request.extend_from_slice(&content);

    let origin_file_data = Arc::new(origin_file_data);

    test_request(
        9097,
        &request,
        move |request| {
            assert_eq!(request.method(), "POST");
            assert_eq!(request.path(), "/form");
            assert_eq!(request.version(), &HttpVersion::Http1_1);

            enum CurrentPart {
                None,
                Field1(Vec<u8>),
                Field2(Vec<u8>),
                File(Vec<u8>),
            }

            let mut current_part = CurrentPart::None;

            let origin_file_data = origin_file_data.clone();

            let mut multipart = MultipartParser::new(&request).unwrap();
            let mut ok = false;
            let mut fifnished = false;
            request.read_content(move |data, complete| {
                multipart.push(data, |ev| {
                    match ev {
                        MultipartParserEvent::Disposition(disposition) => {
                            match &mut current_part {
                                CurrentPart::None => {
                                    assert_eq!(disposition.raw(), b"Content-Disposition: form-data; name=\"field\"");
                                    current_part = CurrentPart::Field1(Vec::new());
                                },
                                CurrentPart::Field1(data) => {
                                    assert_eq!(disposition.raw(), b"Content-Disposition: form-data; name=\"field2\"");
                                    assert_eq!(data, b"text");
                                    current_part = CurrentPart::Field2(Vec::new());
                                },
                                CurrentPart::Field2(data) => {
                                    assert_eq!(disposition.raw(), b"Content-Disposition: form-data; name=\"file\"; filename=\"sample.bin\"\r\nContent-Type: application/octet-stream");
                                    assert_eq!(data, b"other text");
                                    current_part = CurrentPart::File(Vec::new());
                                },
                                CurrentPart::File(_data) => {
                                    assert!(false);
                                },
                            }
                        },
                        MultipartParserEvent::Data { data_part, end } => {
                            match &mut current_part {
                                CurrentPart::None => {
                                    assert!(false);
                                },
                                CurrentPart::Field1(data) => {
                                    data.extend_from_slice(data_part);
                                    if end {
                                        assert_eq!(data, b"text");
                                    }
                                },
                                CurrentPart::Field2(data) => {
                                    data.extend_from_slice(data_part);
                                    if end {
                                        assert_eq!(data, b"other text");
                                    }
                                },
                                CurrentPart::File(data) => {
                                    data.extend_from_slice(data_part);
                                    if end {
                                        assert_eq!(data, origin_file_data.deref());
                                        ok = true;
                                    }
                                },
                            }
                        },
                        MultipartParserEvent::Finished => {
                            match &current_part {
                                CurrentPart::File(data) => {
                                    assert_eq!(data, &*origin_file_data);
                                },
                                _ => assert!(false),
                            }

                            fifnished = true;
                        },
                    }
                })?;

                if let Some(request) = complete {
                    assert!(ok);
                    assert!(fifnished);
                    request.response(200).close().send();
                }

                Ok(())
            });
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
