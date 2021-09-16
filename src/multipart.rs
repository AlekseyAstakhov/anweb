use crate::request::Request;

pub struct MultipartParser {
    state: ParseState,
    buf: Vec<u8>,
    boundary: Vec<u8>,
}

impl MultipartParser {
    pub fn new(request: &Request) -> Result<Self, MultipartError> {
        let content_type_val = request.header_value("Content-Type").unwrap_or("");
        if content_type_val.is_empty() {
            return Err(MultipartError::NoContentTypeHeader);
        }

        let boundary_index = match content_type_val.find("boundary=") {
            None => return Err(MultipartError::NoBoundaryInContentTypeHeader),
            Some(index) => index,
        };

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

                    if let Some((boundary_pos, closing_boundary)) = self.find_boundary(&self.buf) {
                        self.state = ParseState::FindDisposition;

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
                ParseState::FindDisposition => {
                    if let Some(pos) = self.buf.windows(4).position(|win| win == b"\r\n\r\n") {
                        let raw_disposition = &self.buf[0..pos];
                        f(MultipartParserEvent::Disposition(&Disposition { raw: raw_disposition }));
                        self.buf = Vec::from(&self.buf[pos + 4..]);
                        self.state = ParseState::ReadData;
                        continue;
                    }

                    break; // need more data
                }
                ParseState::ReadData => {
                    if let Some((boundary_pos, closing_boundary)) = self.find_boundary(&self.buf) {
                        let data_part = &self.buf[..boundary_pos - 4];
                        if !data_part.is_empty() {
                            f(MultipartParserEvent::Data { data_part, end: true });
                        }

                        self.state = ParseState::FindDisposition;

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

    fn find_boundary(&self, buf: &[u8]) -> Option<(usize, bool/*closing boundary*/)> {
        if buf.len() >= self.boundary.len() + 4 {
            if let Some(pos) = buf.windows(2).position(|win| win == b"--") {
                let boundary_pos = pos + 2;
                if boundary_pos + self.boundary.len() + 2 < buf.len() {
                    if &buf[boundary_pos..boundary_pos + self.boundary.len()] == self.boundary {
                        if &buf[boundary_pos + self.boundary.len()..boundary_pos + self.boundary.len() + 2] == b"\r\n" {
                            // --BOUNDARY\r\n
                            return Some((boundary_pos, false));
                        } else if &buf[boundary_pos + self.boundary.len()..boundary_pos + self.boundary.len() + 2] == b"--" {
                            // --BOUNDARY--\r\n
                            return Some((boundary_pos, true));
                        }
                    }
                }
            }
        }

        None
    }
}

pub struct Disposition<'a> {
    pub raw: &'a [u8],
}

pub enum MultipartParserEvent<'a> {
    Disposition(&'a Disposition<'a>),
    Data { data_part: &'a [u8], end: bool },
    Finished,
}

enum ParseState {
    FindFirstBoundary,
    FindDisposition,
    ReadData,
}

#[derive(Debug)]
pub enum MultipartError {
    NoContentTypeHeader,
    NoBoundaryInContentTypeHeader,
    EmptyBoundaryInHeader,
    // By RFC 2046, boundary must be no longer than 70 characters
    BoundaryLenLimit { len: usize },
}

impl std::fmt::Display for MultipartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}
impl std::error::Error for MultipartError {}
