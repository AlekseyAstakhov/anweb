use crate::request::Request;

pub struct Part<'a> {
    pub disposition: Disposition<'a>,
    pub data: &'a [u8],
}

pub struct Disposition<'a> {
    pub raw: &'a [u8],
}

/// Returns parse multipart result.
pub fn multipart<'a>(content: &'a [u8], request: &Request) -> Result<Vec<Part<'a>>, MultipartError> {
    let content_type_val = request.header_value("Content-Type").unwrap_or("");
    if content_type_val.is_empty() {
        return Err(MultipartError);
    }

    let boundary_index = match content_type_val.find("boundary=") {
        None => return Err(MultipartError),
        Some(index) => index,
    };

    let boundary = &content_type_val[boundary_index + 9..];
    if boundary.is_empty() {
        return Err(MultipartError);
    }

    enum ParseState {
        FindDisposition,
        ParseHeaders(usize),
        ParseContent(usize),
    }

    let mut parse_state = ParseState::FindDisposition;

    let mut parts = Vec::new();

    for i in 0..content.len() {
        match parse_state {
            ParseState::FindDisposition => {
                if i > boundary.len() + 1 && &content[i - 1 - boundary.len()..i - 1] == boundary.as_bytes() {
                    parse_state = ParseState::ParseHeaders(i + 1);
                }
            }
            ParseState::ParseHeaders(disposition_index) => {
                if i - disposition_index > 3 && &content[i - 3..=i] == b"\r\n\r\n" {
                    let raw_disposition = &content[disposition_index..i - 3];
                    parts.push(Part {
                        disposition: Disposition { raw: raw_disposition },
                        data: b"",
                    });
                    parse_state = ParseState::ParseContent(i + 1);
                }
            }
            ParseState::ParseContent(content_index) => {
                if i > boundary.len() + 1 && &content[i - 1..=i] == b"\r\n" && &content[i - 1 - boundary.len()..i - 1] == boundary.as_bytes() {
                    let part_data = &content[content_index..i - 1 - boundary.len() - 4];
                    parse_state = ParseState::ParseHeaders(i + 1);
                    if let Some(last_part) = parts.last_mut() {
                        last_part.data = part_data;
                    }
                } else if i > boundary.len() + 3 && &content[i - 3..=i] == b"--\r\n" && &content[i - 3 - boundary.len()..i - 3] == boundary.as_bytes() {
                    let part_data = &content[content_index..i - boundary.len() - 7];
                    if let Some(last_part) = parts.last_mut() {
                        last_part.data = part_data;
                    }

                    return Ok(parts);
                }
            }
        }
    }

    Err(MultipartError)
}

#[derive(Debug)]
pub struct MultipartError;

impl std::fmt::Display for MultipartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}
impl std::error::Error for MultipartError {}
