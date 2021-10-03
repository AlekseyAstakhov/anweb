use sha1::{Digest, Sha1};
use crate::tcp_session::TcpSession;

pub const CONTINUATION_OPCODE: u8 = 0x0;
pub const TEXT_OPCODE: u8 = 0x1;
pub const BINARY_OPCODE: u8 = 0x2;
pub const CLOSE_OPCODE: u8 = 0x8;

#[derive(Clone)]
pub struct Websocket {
    tcp_session: TcpSession,
}

impl Websocket {
    // Set callback that will called every time a datagram is received
    // or some error such as read/write sock errors or parsing frames.
    pub fn on_frame(&self, callback: impl FnMut(WebsocketResult, Websocket) -> Result<(), WebsocketError> + Send + 'static) {
        if let Ok(mut websocket_callback) = self.tcp_session.inner.websocket_callback.lock() {
            *websocket_callback = Some(Box::new(callback));
        }
    }

    /// Send frame.
    pub fn send(&self, opcode: u8, payload: &[u8]) {
        self.tcp_session.send(&frame(opcode, payload));
    }

    /// Send frame.
    /// # Arguments
    /// * `res_callback` - function that will be called when the write is finished or socket writing error.
    pub fn try_send(&self, opcode: u8, payload: &[u8], res_callback: impl FnMut(Result<(), std::io::Error>) + Send + 'static) {
        self.tcp_session.try_send(&frame(opcode, payload), res_callback);
    }

    /// Close of client socket. After clossing will be generated `sever::Event::Disconnected`.
    pub fn close(&self) {
        self.tcp_session.close()
    }

    /// Returns reference to the TCP session of this websocket.
    pub fn tcp_session(&self) -> &TcpSession {
        &self.tcp_session
    }

    pub(crate) fn new(tcp_session: TcpSession) -> Self {
        Websocket { tcp_session }
    }
}

/// Received websocket frame or error receiving it
pub type WebsocketResult<'a> = Result<&'a Frame, WebsocketError>;

/// Error of websocket such as parsing frame or read from socket.
#[derive(Debug)]
pub enum WebsocketError {
    /// Read from sock error.
    ReadError(std::io::Error),
    /// Error of parsing data.
    ParseFrameError(ParseFrameError),
    /// Register in poll error.
    PollRegisterError(std::io::Error),
}

#[derive(Debug)]
pub enum WebsocketHandshakeError {
    NoSecWebSocketKeyHeader
}

/// Returns hashed key for Sec-WebSocket-Accept header websocket handshake response
/// by Sec-WebSocket-Key of upgrade websocket request.
pub fn accept_key(sec_websocket_key: &str) -> Result<String, WebsocketHandshakeError> {
    let mut hasher = Sha1::new();
    const MAGIC_STRING_FOR_HANDSHAKE: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    hasher.input((sec_websocket_key.to_owned() + MAGIC_STRING_FOR_HANDSHAKE).as_bytes());
    let accept_sha1 = hasher.result();
    Ok(base64::encode(&accept_sha1))
}

/// Make vector containing frame based on the specified opcode and payload data.
pub fn frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let data_len = payload.len();
    const MAX_FRAME_HEADER_LEN: usize = 14;
    let mut result = Vec::with_capacity(MAX_FRAME_HEADER_LEN + data_len);

    let first_byte = opcode | 0b1000_0000;
    result.push(first_byte);

    if data_len < 126 {
        result.push(data_len as u8);
    } else if data_len <= u16::MAX as usize {
        result.push(126);
        let bytes = (data_len as u64).to_be_bytes();
        result.extend_from_slice(&bytes[6..8]);
    } else {
        result.push(127);
        let bytes = (data_len as u64).to_be_bytes();
        result.extend_from_slice(&bytes);
    }

    result.extend_from_slice(payload);

    result
}

/// The parser need to be recreated only after error! Here is not all of things from RFC: 6455
pub struct Parser {
    state: ParserState,
    frame: Frame,
}

impl Parser {
    /// The parser need to be recreated only after error!
    pub fn new() -> Self {
        Parser::default()
    }

    /// Add incoming data for processing.
    pub fn parse_yet(&mut self, tmp_buf: &[u8], payload_limit: usize) -> Result<Option<(Frame, Vec<u8>)>, ParseFrameError> {
        self.frame.buf.extend_from_slice(tmp_buf);
        loop {
            match self.state {
                ParserState::ParseFirstByteWhereFinAndOpcode => {
                    if !self.frame.buf.is_empty() {
                        let first_byte = self.frame.buf[0];
                        self.frame.fin = first_byte & 0b1000_0000 > 0;
                        self.frame.opcode = first_byte & 0b0000_1111;
                        match self.frame.opcode {
                            0x0..=0xF => (),
                            _ => return Err(ParseFrameError::UnsupportedOpcode),
                        }

                        self.state = ParserState::ParseSecondByteWhereMaskAndPayloadLen;
                        continue;
                    }

                    break; // need more data
                }
                ParserState::ParseSecondByteWhereMaskAndPayloadLen => {
                    if self.frame.buf.len() > 1 {
                        let second_byte = self.frame.buf[1];
                        let mask = second_byte & 0b1000_0000;
                        // RFC: 6455 section 5.1: server must disconnect from a client
                        // if that client sends an unmasked message
                        if mask == 0 {
                            return Err(ParseFrameError::UnmaskedClientMaessage);
                        }

                        self.frame.payload_len = (second_byte & 0b0111_1111) as usize;
                        if self.frame.payload_len > payload_limit {
                            return Err(ParseFrameError::PayloadLimit);
                        }

                        if self.frame.payload_len < 126 {
                            self.frame.masking_key_index = 2;
                            self.state = ParserState::ParseMaskingKey;
                            continue;
                        }

                        self.state = ParserState::ParseExtendedPayloadLen;
                        continue;
                    }

                    break; // need more data
                }
                ParserState::ParseMaskingKey => {
                    const MASKING_KEY_LEN: usize = 4;
                    if self.frame.buf.len() >= self.frame.masking_key_index + MASKING_KEY_LEN {
                        self.frame.payload_index = self.frame.masking_key_index + MASKING_KEY_LEN;

                        self.state = ParserState::LoadPayloadData;
                        continue;
                    }

                    break; // need more data
                }
                ParserState::ParseExtendedPayloadLen => {
                    if self.frame.payload_len == 126 {
                        if self.frame.buf.len() < 4 {
                            break; // need more data
                        }

                        let hi = self.frame.buf[2];
                        let low = self.frame.buf[3];
                        let len = hi as usize;
                        let len = len << 8;
                        let len = len | low as usize;

                        if len > payload_limit {
                            return Err(ParseFrameError::PayloadLimit);
                        }

                        self.frame.payload_len = len;
                        self.frame.masking_key_index = 4;
                    } else {
                        if self.frame.buf.len() < 10 {
                            break; // need more data
                        }

                        let mut len = self.frame.buf[2] as usize;
                        for i in 2..10 {
                            len <<= 8;
                            len |= self.frame.buf[i] as usize;
                        }

                        if len > payload_limit {
                            return Err(ParseFrameError::PayloadLimit);
                        }

                        self.frame.payload_len = len;
                        self.frame.masking_key_index = 10;
                    }

                    self.state = ParserState::ParseMaskingKey;
                    continue;
                }
                ParserState::LoadPayloadData => {
                    let frame_len = self.frame.payload_index + self.frame.payload_len;
                    if self.frame.buf.len() >= frame_len {
                        let mut result = Frame::new();
                        std::mem::swap(&mut result, &mut self.frame);

                        let surplus = result.buf[frame_len..].to_vec();
                        result.buf.truncate(frame_len);

                        // mask is checked early. RFC: 6455 section 5.1: server must disconnect
                        // from a client if that client sends an unmasked message
                        let mut mask = [0; 4];
                        mask.clone_from_slice(result.mask().unwrap_or_else(|| {
                            // unreachable code
                            &[0, 0, 0, 0]
                        }));

                        // decode
                        for (i, ch) in result.buf.iter_mut().skip(result.payload_index).enumerate() {
                            *ch ^= mask[i % 4];
                        }

                        self.state = ParserState::ParseFirstByteWhereFinAndOpcode;
                        return Ok(Some((result, surplus)));
                    }

                    break; // need more data
                }
            }
        }

        Ok(None)
    }
}

impl Default for Parser {
    fn default() -> Self {
        Parser {
            frame: Frame::new(),
            state: ParserState::ParseFirstByteWhereFinAndOpcode,
        }
    }
}

/// Parsed websocket frame. See RFC: 6455 section 5.2, Base Framing Protocol.
/// No mask because server accept only frames where mask==1.
#[derive(Debug)]
pub struct Frame {
    /// First bit of first byte.
    /// Indicates that this is the final fragment in a message.
    /// The first fragment MAY also be the final fragment.
    fin: bool,
    /// Last 4 bits of first byte.
    /// Defines the interpretation of the "Payload data".
    /// If an unknown opcode is received, t
    /// he receiving endpoint MUST _Fail the WebSocket Connection_.
    opcode: u8,

    /// Buffer accumulating incoming data.
    buf: Vec<u8>,

    /// Index of payload data in incoming data buffer.
    payload_index: usize,
    /// Length of payload data.
    payload_len: usize,
    /// Index of masking key in incoming data buffer.
    masking_key_index: usize,
}

impl Frame {
    /// Payload.
    pub fn payload(&self) -> &[u8] {
        &self.buf[self.payload_index..self.payload_index + self.payload_len]
    }

    /// First bit of first byte.
    /// Indicates that this is the final fragment in a message.
    /// The first fragment MAY also be the final fragment.
    pub fn fin(&self) -> bool {
        self.fin
    }

    /// Last 4 bits of first byte. Defines the interpretation of the "Payload data".
    /// If an unknown opcode is received,
    /// the receiving endpoint MUST _Fail the WebSocket Connection_.
    pub fn opcode(&self) -> u8 {
        self.opcode
    }

    /// Mask.
    pub fn mask(&self) -> Option<&[u8]> {
        if self.masking_key_index == 0 || self.masking_key_index + 4 > self.buf.len() {
            return None;
        }

        Some(&self.buf[self.masking_key_index..self.masking_key_index + 4])
    }

    /// Raw data buffer of frame.
    pub fn raw(&self) -> &[u8] {
        &self.buf
    }

    /// Opcode is text. It does not guarantee that payload is valid utf-8 string. See RFC: 6455 section 5.2, Base Framing Protocol
    pub fn is_text(&self) -> bool {
        self.opcode == TEXT_OPCODE
    }

    /// Opcode is binary. See RFC: 6455 section 5.2, Base Framing Protocol
    pub fn is_binary(&self) -> bool {
        self.opcode == BINARY_OPCODE
    }

    /// Opcode is continuation. See RFC: 6455 section 5.2, Base Framing Protocol
    pub fn is_continuation(&self) -> bool {
        self.opcode == CONTINUATION_OPCODE
    }

    /// Opcode is close. See RFC: 6455 section 5.2, Base Framing Protocol
    pub fn is_close(&self) -> bool {
        self.opcode == CLOSE_OPCODE
    }

    /// Conditionally uninitialized frame data.
    fn new() -> Self {
        Frame {
            fin: false,
            opcode: 0,
            buf: Vec::new(),
            payload_index: 0,
            payload_len: 0,
            masking_key_index: 0,
        }
    }
}

enum ParserState {
    ParseFirstByteWhereFinAndOpcode,
    ParseSecondByteWhereMaskAndPayloadLen,
    ParseExtendedPayloadLen,
    ParseMaskingKey,
    LoadPayloadData,
}

#[derive(Debug)]
pub enum ParseFrameError {
    UnsupportedOpcode,
    UnmaskedClientMaessage,
    PayloadLimit,
}


impl From<std::io::Error> for WebsocketError {
    fn from(err: std::io::Error) -> Self {
        WebsocketError::ReadError(err)
    }
}

impl From<ParseFrameError> for WebsocketError {
    fn from(err: ParseFrameError) -> Self {
        WebsocketError::ParseFrameError(err)
    }
}

impl std::fmt::Display for WebsocketHandshakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for WebsocketHandshakeError {
}
