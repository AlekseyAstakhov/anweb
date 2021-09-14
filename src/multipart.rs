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
            return Err(MultipartError::ContentTypeHeaderError);
        }

        let boundary_index = match content_type_val.find("boundary=") {
            None => return Err(MultipartError::ContentTypeHeaderError),
            Some(index) => index,
        };

        let boundary = &content_type_val[boundary_index + 9..];
        if boundary.is_empty() {
            return Err(MultipartError::ContentTypeHeaderError);
        }

        Ok(Self {
            state: ParseState::FindBoundary,
            buf: vec![],
            boundary: Vec::from(boundary),
        })
    }

    pub fn push(&mut self, data: &[u8], mut f: impl FnMut(MultipartParserEvent)) -> Result<(), MultipartError> {
        let pos = self.buf.len();
        self.buf.extend_from_slice(data);

        for i in pos..self.buf.len() {
            match self.state {
                ParseState::FindBoundary => {
                    if i > self.boundary.len() + 1 && &self.buf[i - self.boundary.len() - 1..i - 1] == self.boundary {
                        self.state = ParseState::FindDisposition(i + 1);
                    }
                }
                ParseState::FindDisposition(disposition_index) => {
                    if i - disposition_index > 3 && &self.buf[i - 3..=i] == b"\r\n\r\n" {
                        let raw_disposition = &self.buf[disposition_index..i - 3];
                        self.state = ParseState::ReadData(i + 1);

                        f(MultipartParserEvent::Disposition( &Disposition { raw: raw_disposition }) );
                    }
                }
                ParseState::ReadData(content_index) => {
                    if i > self.boundary.len() + 1 && &self.buf[i - 1..=i] == b"\r\n" && &self.buf[i - 1 - self.boundary.len()..i - 1] == self.boundary {
                        let part_data = &self.buf[content_index..i - 1 - self.boundary.len() - 4];
                        self.state = ParseState::FindDisposition(i + 1);

                        f(MultipartParserEvent::Data { data: part_data, end: false });

                    } else if i > self.boundary.len() + 3 && &self.buf[i - 3..=i] == b"--\r\n" && &self.buf[i - 3 - self.boundary.len()..i - 3] == self.boundary {
                        let part_data = &self.buf[content_index..i - self.boundary.len() - 7];

                        f(MultipartParserEvent::Data { data: part_data, end: true });
                    }
                }
            }
        }

        Ok(())
    }
}

pub struct Disposition<'a> {
    pub raw: &'a [u8],
}

pub enum MultipartParserEvent<'a, 'b> {
    Disposition(&'a Disposition<'a>),
    Data { data: &'b [u8], end: bool },
}

enum ParseState {
    FindBoundary,
    FindDisposition(usize),
    ReadData(usize),
}

#[derive(Debug)]
pub enum MultipartError {
    ContentTypeHeaderError,
}

impl std::fmt::Display for MultipartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}
impl std::error::Error for MultipartError {}
