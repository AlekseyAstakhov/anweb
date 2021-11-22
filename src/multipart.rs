use crate::request::Request;

pub struct MultipartParser {
    state: ParseState,
    buf: Vec<u8>,
    boundary: Vec<u8>,
}

impl MultipartParser {
    /// Returns new multipart parser.
    pub fn new(request: &Request) -> Result<Self, MultipartError> {
        let content_type_val = request.header_value("Content-Type").unwrap_or("");
        if content_type_val.is_empty() {
            return Err(MultipartError::NoContentTypeHeader);
        }

        let boundary_index = content_type_val.find("boundary=")
            .ok_or(MultipartError::NoBoundaryInContentTypeHeader)?;

        let boundary = Vec::from(&content_type_val[boundary_index + 9..]);
        if boundary.is_empty() {
            return Err(MultipartError::EmptyBoundaryInHeader);
        }

        // Boundary must be no longer than 70 characters RFC 2046
        if boundary.len() > 70 {
            return Err(MultipartError::BoundaryLenLimit { len: boundary.len() });
        }

        Ok(Self {
            state: ParseState::FindFirstBoundary,
            buf: vec![],
            boundary,
        })
    }

    /// Add data for parsing.
    pub fn push(&mut self, data: &[u8], mut f: impl FnMut(MultipartParserEvent)) -> Result<(), MultipartError> {
        self.buf.extend_from_slice(data);

        let boundary_detect_len = self.boundary.len() + 4;

        loop {
            match self.state {
                ParseState::FindFirstBoundary => {
                    // There appears to be room for additional information prior to the first
                    // encapsulation boundary and following the final boundary.
                    // These areas should generally be left blank, and implementations should
                    // ignore anything that appears before the first boundary or after the last one.
                    // (RFC 2046)

                    if let Some((boundary_pos, closing_boundary)) = find_boundary(&self.buf, &self.boundary) {
                        self.state = ParseState::Disposition;

                        if closing_boundary {
                            // This is not explicitly defined in the RFC 2046, but browsers send
                            // closing boundary delimiter when multiform not contains parts at all
                            f(MultipartParserEvent::Finished);
                            self.buf.clear();
                            break;
                        }

                        self.buf = Vec::from(&self.buf[boundary_pos + self.boundary.len() + 2..]);
                        continue;
                    }

                    if self.buf.len() > boundary_detect_len * 2 {
                        self.buf = Vec::from(&self.buf[self.buf.len() - boundary_detect_len * 2..]);
                    }

                    break; // need more data
                }
                ParseState::Disposition => {
                    if self.buf.len() > 4 {
                        if let Some(pos) = self.buf.windows(4).position(|win| win == b"\r\n\r\n") {
                            let left = if &self.buf[0..2] != b"\r\n" { 0 } else { 2 };
                            let raw_disposition = &self.buf[left..pos];
                            f(MultipartParserEvent::Disposition(&Disposition { raw: raw_disposition }));
                            self.buf = Vec::from(&self.buf[pos + 4..]);
                            self.state = ParseState::ReadData;
                            continue;
                        }
                    }

                    break; // need more data
                }
                ParseState::ReadData => {
                    if let Some((boundary_pos, closing_boundary)) = find_boundary(&self.buf, &self.boundary) {
                        let data_part = &self.buf[..boundary_pos - 2]; // checked in find_boundary function
                        if !data_part.is_empty() {
                            f(MultipartParserEvent::Data { data_part, end: true });
                        }

                        self.state = ParseState::Disposition;

                        if closing_boundary {
                            f(MultipartParserEvent::Finished);
                            self.buf.clear();
                            break; // Finish
                        }

                        self.buf = Vec::from(&self.buf[boundary_pos + self.boundary.len()..]);
                        continue;
                    }

                    let data_part = &self.buf;
                    f(MultipartParserEvent::Data { data_part, end: false });
                    self.buf.clear();
                    break; // need more data
                }
            }
        }

        Ok(())
    }
}

fn find_boundary(buf: &[u8], boundary: &[u8]) -> Option<(usize, bool/*closing boundary*/)> {
    if buf.len() >= boundary.len() + 4 {
        if let Some(pos) = buf.windows(2).position(|win| win == b"--") {
            let boundary_pos = pos + 2;
            if buf.len() >= boundary_pos + boundary.len() + 2 {
                if &buf[boundary_pos..boundary_pos + boundary.len()] == boundary {
                    if &buf[boundary_pos + boundary.len()..boundary_pos + boundary.len() + 2] == b"\r\n" {
                        // --BOUNDARY\r\n
                        return Some((boundary_pos, false));
                    } else if &buf[boundary_pos + boundary.len()..boundary_pos + boundary.len() + 2] == b"--" {
                        // --BOUNDARY--
                        return Some((boundary_pos, true));
                    }
                }
            }
        }
    }

    None
}

/// Disposition of multipart part.
#[derive(Debug)]
pub struct Disposition<'a> {
    raw: &'a [u8],
}

impl<'a> Disposition<'a>  {
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
}

/// Event of multipart parser.
pub enum MultipartParserEvent<'a> {
    /// New disposition found.
    Disposition(&'a Disposition<'a>),
    /// Data part. If end false then this part of part.
    Data { data_part: &'a [u8], end: bool },
    ///  All parts received (last boundary that with "--" postfix found).
    Finished,
}

enum ParseState {
    FindFirstBoundary,
    Disposition,
    ReadData,
}

#[derive(Debug)]
pub enum MultipartError {
    /// No "Content-Type" header in HTTP request.
    NoContentTypeHeader,
    /// No "boundary=" in value of "Content-Type" header.
    NoBoundaryInContentTypeHeader,
    /// Boundary in value of "Content-Type" header is empty.
    EmptyBoundaryInHeader,
    /// By RFC 2046, boundary must be no longer than 70 characters.
    BoundaryLenLimit { len: usize },
}

impl std::fmt::Display for MultipartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}
impl std::error::Error for MultipartError {}
