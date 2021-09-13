use crate::tcp_session::InnerTcpSession;
use crate::websocket::{ParseFrameError, ParsedFrame};
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Clone)]
pub struct WebsocketSession {
    pub(crate) inner: Arc<InnerTcpSession>,
}

impl WebsocketSession {
    /// Client id on server in connection order.
    pub fn id(&self) -> u64 {
        self.inner.id()
    }

    /// An internet socket address, either IPv4 or IPv6.
    pub fn addr(&self) -> &SocketAddr {
        &self.inner.addr
    }

    pub fn send(&mut self, data: &[u8]) {
        self.inner.send(data);
    }

    /// Close of client socket. After clossing will be generated `sever::Event::Disconnected`.
    pub fn disconnect(&self) {
        self.inner.disconnect()
    }
}

/// Received websocket frame or error receiving it
pub type WebsocketResult<'a> = Result<&'a ParsedFrame, WebsocketError>;

/// Error of websocket such as parsing frame or read from socket.
pub enum WebsocketError {
    ParseFrameError(ParseFrameError),
    StreamError(std::io::Error),
}

impl From<std::io::Error> for WebsocketError {
    fn from(err: std::io::Error) -> Self {
        WebsocketError::StreamError(err)
    }
}

impl From<ParseFrameError> for WebsocketError {
    fn from(err: ParseFrameError) -> Self {
        WebsocketError::ParseFrameError(err)
    }
}
