use crate::request::Request;
use sha1::{Digest, Sha1};
use std::u16;

pub const CONTINUATION_OPCODE: u8 = 0x0;
pub const TEXT_OPCODE: u8 = 0x1;
pub const BINARY_OPCODE: u8 = 0x2;
pub const CLOSE_OPCODE: u8 = 0x8;

/// Generate response to upgrade connection request. Check only Sec-WebSocket-Key header of request!
pub fn handshake_response(request: &Request) -> Result<Vec<u8>, HandshakeError> {
    const MAGIC_STRING_FOR_HANDSHAKE: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

    if let Some(key) = request.header_value("Sec-WebSocket-Key") {
        let mut hasher = Sha1::new();
        hasher.input((key.to_owned() + MAGIC_STRING_FOR_HANDSHAKE).as_bytes());
        let accept_sha1 = hasher.result();
        let accept = base64::encode(&accept_sha1);

        let protocol = if let Some(protocol) = request.header_value("Sec-WebSocket-Protocol") { format!("Sec-WebSocket-Protocol: {}\r\n", &protocol) } else { String::new() };

        let response = format!(
            "{} 101 Switching Protocols\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Accept: {}\r\n\
             {}\
             \r\n",
            request.version.to_string_for_response(),
            &accept,
            &protocol
        );

        return Ok(Vec::from(response));
    }

    Err(HandshakeError {})
}

#[derive(Debug)]
pub struct HandshakeError {}

impl std::fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for HandshakeError {}

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
        result.push(bytes[6]);
        result.push(bytes[7]);
    } else {
        result.push(127);
        let bytes = (data_len as u64).to_be_bytes();
        result.push(bytes[0]);
        result.push(bytes[1]);
        result.push(bytes[2]);
        result.push(bytes[3]);
        result.push(bytes[4]);
        result.push(bytes[5]);
        result.push(bytes[6]);
        result.push(bytes[7]);
    }

    result.extend_from_slice(payload);

    result
}

/// Parse websocket frame. See RFC: 6455 section 5.2, Base Framing Protocol. No mask because server accept only frames where mask==1.
#[derive(Debug)]
pub struct ParsedFrame {
    /// FIN: First bit of first byte. Indicates that this is the final fragment in a message. The first fragment MAY also be the final fragment.
    fin: bool,
    /// opcode: Last 4 bits of first byte. Defines the interpretation of the "Payload data". If an unknown opcode is received, the receiving endpoint MUST _Fail the WebSocket Connection_.
    opcode: u8,

    /// Full first byte of frame.
    first_byte: u8,

    /// Buffer accumulating incoming data.
    buf: Vec<u8>,

    /// Index of payload data in incoming data buffer.
    payload_index: usize,
    /// Length of payload data.
    payload_len: usize,
    /// Index of masking key in incoming data buffer.
    masking_key_index: usize,
}

/// The parser need to be recreated only after error! Here is not all of things from RFC: 6455
pub struct Parser {
    state: ParserState,
    frame: ParsedFrame,
}

impl ParsedFrame {
    /// See RFC: 6455 section 5.2, Base Framing Protocol
    pub fn payload(&self) -> &[u8] {
        &self.buf[self.payload_index..self.payload_index + self.payload_len]
    }

    /// See RFC: 6455 section 5.2, Base Framing Protocol
    pub fn opcode(&self) -> u8 {
        self.opcode
    }

    /// See RFC: 6455 section 5.2, Base Framing Protocol
    pub fn mask(&self) -> Option<&[u8]> {
        if self.masking_key_index == 0 || self.masking_key_index + 4 > self.buf.len() {
            return None;
        }

        Some(&self.buf[self.masking_key_index..self.masking_key_index + 4])
    }

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
        ParsedFrame {
            fin: false,
            opcode: 0,
            first_byte: 0,
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

impl Parser {
    /// The parser need to be recreated only after error!
    pub fn new() -> Self {
        Parser::default()
    }

    /// Add incoming data for processing.
    pub fn parse_yet(&mut self, tmp_buf: &[u8], payload_limit: usize) -> Result<Option<(ParsedFrame, Vec<u8>)>, ParseFrameError> {
        self.frame.buf.extend_from_slice(tmp_buf);
        loop {
            match self.state {
                ParserState::ParseFirstByteWhereFinAndOpcode => {
                    if !self.frame.buf.is_empty() {
                        let first_byte = self.frame.buf[0];
                        self.frame.first_byte = first_byte;
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
                        // RFC: 6455 section 5.1: server must disconnect from a client if that client sends an unmasked message
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
                        let mut result = ParsedFrame::new();
                        std::mem::swap(&mut result, &mut self.frame);

                        let surplus = result.buf[frame_len..].to_vec();
                        result.buf.truncate(frame_len);

                        // mask is checked early. RFC: 6455 section 5.1: server must disconnect from a client if that client sends an unmasked message
                        let mut mask = [0; 4];
                        mask.clone_from_slice(result.mask().unwrap_or_else(|| {
                            dbg!("unreachable code");
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
            frame: ParsedFrame::new(),
            state: ParserState::ParseFirstByteWhereFinAndOpcode,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let incoming_data = [];
        let mut parser = Parser::new();
        if let Ok(result) = parser.parse_yet(&incoming_data, 100) {
            assert!(result.is_none());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn part_of_frame() {
        let incoming_data = [129, 140, 211, 25, 248, 86];
        let mut parser = Parser::new();
        if let Ok(result) = parser.parse_yet(&incoming_data, 100) {
            assert!(result.is_none());
        } else {
            assert!(false);
        }
    }

    #[test]
    fn one_good_frame() {
        let incoming_data = [129, 140, 211, 25, 248, 86, 155, 124, 148, 58, 188, 57, 143, 57, 161, 117, 156, 119];
        let mut parser = Parser::new();
        if let Ok(result) = parser.parse_yet(&incoming_data, 12) {
            if let Some((frame, surplus)) = result {
                assert_eq!(frame.fin, true);
                assert_eq!(frame.opcode, 1);
                assert_eq!(frame.first_byte, 129);
                assert_eq!(frame.buf, [129, 140, 211, 25, 248, 86, 72, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100, 33]);
                assert_eq!(frame.payload_index, 6);
                assert_eq!(frame.payload_len, 12);
                assert_eq!(frame.masking_key_index, 2);
                assert!(frame.is_text());
                assert_eq!(frame.payload(), b"Hello world!");
                assert!(surplus.is_empty());
            } else {
                // because data contains full frame
                assert!(false);
            }
        } else {
            assert!(false);
        }
    }

    #[test]
    fn two_good_frame_and_surplus() {
        let incoming_data = [129, 131, 216, 213, 165, 109, 233, 231, 150];
        let mut parser = Parser::new();
        if let Ok(result) = parser.parse_yet(&incoming_data, 100) {
            if let Some((frame, surplus)) = result {
                assert_eq!(frame.fin, true);
                assert_eq!(frame.opcode, 1);
                assert_eq!(frame.first_byte, 129);
                assert_eq!(frame.buf, [129, 131, 216, 213, 165, 109, 49, 50, 51]);
                assert_eq!(frame.payload_index, 6);
                assert_eq!(frame.payload_len, 3);
                assert_eq!(frame.masking_key_index, 2);
                assert_eq!(frame.payload(), b"123");
                assert!(surplus.is_empty());

                let incoming_data = [129, 134, 6, 145, 169, 18, 103, 243, 202, 118, 99, 247, 129, 137];
                if let Ok(result) = parser.parse_yet(&incoming_data, 100) {
                    if let Some((frame, surplus)) = result {
                        assert_eq!(frame.fin, true);
                        assert_eq!(frame.opcode, 1);
                        assert_eq!(frame.first_byte, 129);
                        assert_eq!(frame.buf, [129, 134, 6, 145, 169, 18, 97, 98, 99, 100, 101, 102]);
                        assert_eq!(frame.payload_index, 6);
                        assert_eq!(frame.payload_len, 6);
                        assert_eq!(frame.masking_key_index, 2);
                        assert_eq!(frame.payload(), b"abcdef");
                        assert_eq!(surplus, [129, 137]);
                    } else {
                        // because data contains full frame
                        assert!(false);
                    }
                } else {
                    assert!(false);
                }
            } else {
                // because data contains full frame
                assert!(false);
            }
        } else {
            assert!(false);
        }
    }

    #[test]
    fn two_good_frame_together_and_surplus() {
        let incoming_data = [129, 131, 216, 213, 165, 109, 233, 231, 150, 129, 134, 6, 145, 169, 18, 103, 243, 202, 118, 99, 247, 129, 133];
        let mut parser = Parser::new();
        if let Ok(result) = parser.parse_yet(&incoming_data, 100) {
            if let Some((frame, surplus)) = result {
                assert_eq!(frame.fin, true);
                assert_eq!(frame.opcode, 1);
                assert_eq!(frame.first_byte, 129);
                assert_eq!(frame.buf, [129, 131, 216, 213, 165, 109, 49, 50, 51]);
                assert_eq!(frame.payload_index, 6);
                assert_eq!(frame.payload_len, 3);
                assert_eq!(frame.masking_key_index, 2);
                assert_eq!(frame.payload(), b"123");
                assert!(!surplus.is_empty());

                if let Ok(result) = parser.parse_yet(&surplus, 100) {
                    if let Some((frame, surplus)) = result {
                        assert_eq!(frame.fin, true);
                        assert_eq!(frame.opcode, 1);
                        assert_eq!(frame.first_byte, 129);
                        assert_eq!(frame.buf, [129, 134, 6, 145, 169, 18, 97, 98, 99, 100, 101, 102]);
                        assert_eq!(frame.payload_index, 6);
                        assert_eq!(frame.payload_len, 6);
                        assert_eq!(frame.masking_key_index, 2);
                        assert_eq!(frame.payload(), b"abcdef");
                        assert_eq!(surplus, [129, 133]);
                    } else {
                        // because data contains full frame
                        assert!(false);
                    }
                } else {
                    assert!(false);
                }
            } else {
                // because data contains full frame
                assert!(false);
            }
        } else {
            assert!(false);
        }
    }

    #[test]
    fn close() {
        let incoming_data = [136, 130, 149, 71, 232, 208, 3, 233];
        let mut parser = Parser::new();
        if let Ok(result) = parser.parse_yet(&incoming_data, 100) {
            if let Some((frame, surplus)) = result {
                assert_eq!(frame.fin, true);
                assert_eq!(frame.opcode, 8);
                assert!(frame.is_close());
                assert!(surplus.is_empty());
            } else {
                // because data contains full frame
                assert!(false);
            }
        } else {
            assert!(false);
        }
    }

    #[test]
    fn no_masked_frame_for_send() {
        assert_eq!(frame(TEXT_OPCODE, &[]), [129, 0]);
        assert_eq!(frame(BINARY_OPCODE, &[]), [130, 0]);
        assert_eq!(frame(TEXT_OPCODE, b"1"), [129, 1, 49]);
        assert_eq!(frame(BINARY_OPCODE, b"1"), [130, 1, 49]);
        assert_eq!(frame(TEXT_OPCODE, b"abcdef"), [129, 6, 97, 98, 99, 100, 101, 102]);
        assert_eq!(frame(BINARY_OPCODE, b"abcdef"), [130, 6, 97, 98, 99, 100, 101, 102]);
    }

    #[test]
    fn payload_len_limit() {
        let incoming_data = [129, 140, 211, 25, 248, 86, 155, 124, 148, 58, 188, 57, 143, 57, 161, 117, 156, 119];
        let mut parser = Parser::new();
        if let Err(_) = parser.parse_yet(&incoming_data, 11) {
            assert!(true);
        }
    }
}
