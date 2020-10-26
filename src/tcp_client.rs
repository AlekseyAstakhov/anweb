use crate::http_client::{HttpClient, HttpResult, HttpError};
use crate::websocket_client::{WebsocketClient, WebsocketResult, WebsocketError};
use rustls::Session;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::io;
use std::io::{ErrorKind, Read, Write};
use std::net::SocketAddr;

/// Client connection to the server.
#[derive(Clone)]
pub struct TcpClient {
    /// Private data.
    pub(crate) inner: Arc<InnerTcpClient>,
}

impl TcpClient {
    /// Client id on server in connection order.
    pub fn id(&self) -> u64 {
        self.inner.id()
    }

    /// An internet socket address, either IPv4 or IPv6.
    pub fn addr(&self) -> &SocketAddr {
        &self.inner.addr
    }

    /// Send arbitrary data to the client. Data may not be sent immediately, but in parts.
    pub fn send(&mut self, data: &[u8]) {
        self.inner.send(data);
    }

    /// Send arbitrary shared data to the client. Data may not be sent immediately, but in parts.
    pub fn send_arc(&mut self, data: &Arc<Vec<u8>>) {
        self.inner.send_arc(data);
    }

    /// Close of client socket. After clossing will be generated `sever::Event::Disconnected`.
    pub fn disconnect(&self) {
        self.inner.disconnect();
    }

    /// Set a callback function that is called when a new HTTP request is received or error receiving it.
    pub fn switch_to_http(&self, callback: impl FnMut(HttpResult, HttpClient) -> Result<(), Box<dyn std::error::Error>> + Send + 'static) {
        if let Ok(mut http_request_callback) = self.inner.http_request_callback.lock() {
            *http_request_callback = Some(Box::new(callback));
            self.inner.is_http_mode.store(true, Ordering::SeqCst);
        }
    }

    /// Need close of client socket.
    pub(crate) fn need_disconnect(&self) -> bool {
        self.inner.need_disconnect.load(Ordering::SeqCst)
    }

    /// Return true if client is using for receiving http requests and send responses.
    pub(crate) fn is_http_mode(&self) -> bool {
        self.inner.is_http_mode()
    }

    /// Helps call callback.
    pub(crate) fn call_http_callback(&self, request: HttpResult) {
        if let Ok(mut callback) = self.inner.http_request_callback.lock() {
            if let Some(callback) = &mut *callback {
                if callback(request, HttpClient { inner: self.inner.clone() }).is_err() {
                    self.disconnect();
                }
            }
        }
    }

    /// Helps call callback.
    pub(crate) fn call_raw_content_callback(&self, content: Vec<u8>) {
        if let Ok(mut callback) = self.inner.raw_content_callback.lock() {
            if let Some(callback) = &mut *callback {
                if callback(content, HttpClient { inner: self.inner.clone() }).is_err() {
                    self.disconnect();
                }
            }
        }
    }

    /// Helps call callback.
    pub(crate) fn call_websocket_callback(&self, frame: WebsocketResult) {
        if let Ok(mut callback) = self.inner.websocket_callback.lock() {
            if let Some(callback) = &mut *callback {
                if callback(frame, WebsocketClient { inner: self.inner.clone() }).is_err() {
                    self.disconnect();
                }
            }
        }
    }

    /// Called when new TCP connection.
    pub(crate) fn new(id: u64, slab_key: usize, stream: mio::net::TcpStream, addr: SocketAddr, tls_session: Option<Mutex<rustls::ServerSession>>, mio_poll: Arc<mio::Poll>, http_date_string: Arc<RwLock<String>>) -> Self {
        TcpClient {
            inner: Arc::new(InnerTcpClient {
                id,
                slab_key,
                mio_stream: Mutex::new(stream),
                addr,
                tls_session,
                http_request_callback: Mutex::new(None),
                is_http_mode: Arc::new(AtomicBool::new(false)),
                websocket_callback: Mutex::new(None),
                raw_content_callback: Mutex::new(None),
                need_disconnect: AtomicBool::new(false),
                surpluses_to_write: Mutex::new(Vec::new()),
                mio_poll,
                http_date_string,
                need_disconnect_after_http_response: Arc::new(AtomicBool::new(false)),
            }),
        }
    }

    /// Writes data that was not written in a previous write attempt. Called when the socket is ready to write again.
    pub(crate) fn send_yet(&self) {
        if let Ok(mut surpluses_for_write) = self.inner.surpluses_to_write.lock() {
            // ???
            if surpluses_for_write.is_empty() {
                dbg!("unreachable code");
                if let Ok(stream) = self.inner.mio_stream.lock() {
                    match self.inner.mio_poll.reregister(&*stream, mio::Token(self.inner.slab_key), mio::Ready::readable(), mio::PollOpt::level()) {
                        Ok(()) => {
                            return;
                        }
                        Err(err) => {
                            if self.is_http_mode() {
                                self.call_http_callback(Err(HttpError::StreamError(err)));
                            } else {
                                self.call_websocket_callback(Err(WebsocketError::StreamError(err)));
                            }
                        }
                    }
                }

                self.disconnect();
                return;
            }

            for surplus in surpluses_for_write.iter_mut() {
                // ???
                if surplus.write_yet_cnt >= surplus.data.len() {
                    dbg!("unreachable code");
                    // remove latter from vec below
                    continue;
                }

                match self.inner.write(&surplus.data[surplus.write_yet_cnt..]) {
                    Ok(cnt) => {
                        surplus.write_yet_cnt += cnt;
                        if surplus.write_yet_cnt < surplus.data.len() {
                            // will write latter when writeable
                            break;
                        }
                    }
                    Err(err) => {
                        if err.kind() == std::io::ErrorKind::WouldBlock {
                            // will write latter when writeable
                            break;
                        } else if err.kind() == std::io::ErrorKind::ConnectionReset {
                            self.disconnect();
                        } else {
                            if self.is_http_mode() {
                                self.call_http_callback(Err(HttpError::StreamError(err)));
                            } else {
                                self.call_websocket_callback(Err(WebsocketError::StreamError(err)));
                            }
                            self.disconnect();
                        }
                    }
                }
            }

            surpluses_for_write.retain(|surplus| surplus.write_yet_cnt < surplus.data.len());

            if surpluses_for_write.is_empty() {
                if let Ok(stream) = self.inner.mio_stream.lock() {
                    match self.inner.mio_poll.reregister(&*stream, mio::Token(self.inner.slab_key), mio::Ready::readable(), mio::PollOpt::level()) {
                        Ok(()) => {
                            // all data sent, switch to read mode
                            if self.is_http_mode() && self.inner.need_disconnect_after_http_response.load(Ordering::SeqCst) {
                                self.disconnect();
                            }

                            return;
                        }
                        Err(err) => {
                            if self.is_http_mode() {
                                self.call_http_callback(Err(HttpError::StreamError(err)));
                            } else {
                                self.call_websocket_callback(Err(WebsocketError::StreamError(err)));
                            }
                        }
                    }
                }

                self.disconnect();
            }
        } else {
            dbg!("unreachable code");
            self.disconnect();
        }
    }
}

impl Read for TcpClient {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Write for TcpClient {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Private data of client.
pub(crate) struct InnerTcpClient {
    /// Client id on server in connection order.
    id: u64,
    /// Slab key of this client connection on http server.
    slab_key: usize,
    /// An internet socket address, either IPv4 or IPv6.
    pub(crate) addr: SocketAddr,
    /// Stream which received from MIO event.
    pub(crate) mio_stream: Mutex<mio::net::TcpStream>,
    /// TLS session.
    tls_session: Option<Mutex<rustls::ServerSession>>,

    /// Callback function that is called when a new HTTP request is received or error receiving it.
    pub(crate) http_request_callback: Mutex<Option<Box<dyn FnMut(HttpResult, HttpClient) -> Result<(), Box<dyn std::error::Error>> + Send>>>,
    /// Sets true when callback is set.
    pub(crate) is_http_mode: Arc<AtomicBool>,

    /// Callback function that is called when content of HTTP request is fully received or error receiving it.
    pub(crate) raw_content_callback: Mutex<Option<Box<dyn FnMut(Vec<u8>, HttpClient) -> Result<(), Box<dyn std::error::Error>> + Send>>>,
    /// Callback function that is called when a new websocket frame is received or error receiving it.
    pub(crate) websocket_callback: Mutex<Option<Box<dyn FnMut(WebsocketResult, WebsocketClient) -> Result<(), WebsocketError> + Send>>>,

    /// Data that was not written in one write operation and is waiting for the socket to be ready.
    surpluses_to_write: Mutex<Vec<SurplusForWrite>>,

    /// Mio poll. Need only for reregister client for readable/writable.
    mio_poll: Arc<mio::Poll>,

    /// Determines whether to close connection. Connection will be closed when all other connections with read/write readiness are processing completed.
    need_disconnect: AtomicBool,

    /// Prepared rfc7231 string for http responses, update once per second.
    pub(crate) http_date_string: Arc<RwLock<String>>,

    /// For close the connection after the http response.
    pub(crate) need_disconnect_after_http_response: Arc<AtomicBool>,
}

/// Data that was not written in one write operation and is waiting for the socket to be ready.
struct SurplusForWrite {
    data: Arc<Vec<u8>>,
    write_yet_cnt: usize,
}

/// Private tcp-client data.
impl InnerTcpClient {
    /// Client id on server in connection order.
    pub fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn is_http_mode(&self) -> bool {
        self.is_http_mode.load(Ordering::SeqCst)
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        let readed_cnt = {
            match self.mio_stream.lock() {
                Ok(mut stream) => {
                    //~=~=~=~=~=~=~=~=
                    stream.read(buf)?
                    //~=~=~=~=~=~=~=~=
                }
                Err(err) => {
                    return Err(io::Error::new(ErrorKind::Other, format!("{}", err)));
                }
            }
        };

        match &self.tls_session {
            None => Ok(readed_cnt),
            Some(tls_session) => {
                if readed_cnt == 0 {
                    return Ok(0);
                }

                let read_buf: &mut dyn std::io::Read = &mut &buf[..readed_cnt];
                match tls_session.lock() {
                    Ok(mut tls_session) => {
                        tls_session.read_tls(read_buf)?;

                        if let Err(err) = tls_session.process_new_packets() {
                            return Err(io::Error::new(ErrorKind::Other, err));
                        }

                        let tlse_readed_cnt = tls_session.read(&mut buf[..])?;
                        while tls_session.wants_write() {
                            if let Ok(mut stream) = self.mio_stream.lock() {
                                //=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=
                                tls_session.write_tls(&mut *stream)?;
                                //=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=
                            }
                        }

                        if tlse_readed_cnt == 0 {
                            return Err(io::Error::new(std::io::ErrorKind::WouldBlock, "operation would block"));
                        }

                        Ok(tlse_readed_cnt)
                    }
                    Err(err) => {
                        Err(io::Error::new(ErrorKind::Other, format!("{}", err)))
                    }
                }
            }
        }
    }

    /// Send arbitrary data to the client. Data may not be sent immediately, but in parts.
    pub fn send(&self, data: &[u8]) {
        if let Ok(mut supluses) = self.surpluses_to_write.lock() {
            // already writing, add to the recording queue
            if !supluses.is_empty() {
                supluses.push(SurplusForWrite { data: Arc::new(data.to_vec()), write_yet_cnt: 0 });
                return;
            }
        }

        match self.write(data) {
            Ok(cnt) => {
                if cnt < data.len() {
                    self.send_later(SurplusForWrite { data: Arc::new(data[cnt..].to_vec()), write_yet_cnt: 0 });
                } else {
                    // all data is written
                    if self.is_http_mode() && self.need_disconnect_after_http_response.load(Ordering::SeqCst) {
                        self.disconnect();
                    }
                }
            }
            Err(err) => {
                if err.kind() == std::io::ErrorKind::WouldBlock {
                    self.send_later(SurplusForWrite { data: Arc::new(data.to_vec()), write_yet_cnt: 0 });
                } else if err.kind() == std::io::ErrorKind::ConnectionReset {
                    self.disconnect();
                } else {
                    self.disconnect();
                    dbg!(err);
                }
            }
        }
    }

    /// Send arbitrary shared data to the client. Data may not be sent immediately, but in parts.
    pub fn send_arc(&self, data: &Arc<Vec<u8>>) {
        if let Ok(mut supluses) = self.surpluses_to_write.lock() {
            // already writing, add to the recording queue
            if !supluses.is_empty() {
                supluses.push(SurplusForWrite { data: Arc::clone(data), write_yet_cnt: 0 });
                return;
            }
        }

        match self.write(&data) {
            Ok(cnt) => {
                if cnt < data.len() {
                    self.send_later(SurplusForWrite { data: Arc::clone(data), write_yet_cnt: cnt });
                } else {
                    // all data is written
                    if self.is_http_mode() && self.need_disconnect_after_http_response.load(Ordering::SeqCst) {
                        self.disconnect();
                    }
                }
            }
            Err(err) => {
                if err.kind() == std::io::ErrorKind::WouldBlock {
                    self.send_later(SurplusForWrite { data: Arc::clone(data), write_yet_cnt: 0 });
                } else if err.kind() == std::io::ErrorKind::ConnectionReset {
                    self.disconnect();
                } else {
                    self.disconnect();
                    dbg!(err);
                }
            }
        }
    }

    /// If the data was not sent immediately, it switches to the sending mode in parts.
    fn send_later(&self, surplus: SurplusForWrite) {
        if let Ok(mut supluses) = self.surpluses_to_write.lock() {
            if let Ok(stream) = self.mio_stream.lock() {
                supluses.push(surplus);
                match self.mio_poll.reregister(&*stream, mio::Token(self.slab_key), mio::Ready::writable(), mio::PollOpt::level()) {
                    Ok(()) => {
                        return;
                    }
                    Err(err) => {
                        dbg!(err);
                        self.disconnect();
                        return;
                    }
                }
            }
        }

        dbg!("unreachable code");
        self.disconnect();
    }

    /// Close of client socket. After clossing will be generated `sever::Event::Disconnected`.
    pub fn disconnect(&self) {
        self.need_disconnect.store(true, Ordering::SeqCst);
    }

    fn write(&self, buf: &[u8]) -> io::Result<usize> {
        let tls_session = &self.tls_session;
        let stream = &self.mio_stream;

        match tls_session {
            Some(tls_session) => {
                match tls_session.lock() {
                    Ok(mut tls_session) => {
                        match stream.lock() {
                            Ok(mut stream) => {
                                //~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=
                                let mut cnt = tls_session.write(buf)?;

                                while tls_session.wants_write() {
                                    cnt += tls_session.write_tls(&mut *stream)?;
                                }

                                Ok(cnt)
                                //~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=
                            }
                            Err(err) => {
                                Err(io::Error::new(ErrorKind::Other, format!("{}", err)))
                            }
                        }
                    }
                    Err(err) => {
                        Err(io::Error::new(ErrorKind::Other, format!("{}", err)))
                    }
                }
            }
            None => {
                match stream.lock() {
                    Ok(mut stream) => {
                        //~=~=~=~=~=~=~=~=~=~=~=~=
                        stream.write(buf)
                        //~=~=~=~=~=~=~=~=~=~=~=~=
                    }
                    Err(err) => {
                        Err(io::Error::new(ErrorKind::Other, format!("{}", err)))
                    }
                }
            }
        }
    }

    fn flush(&self) -> io::Result<()> {
        let tls_session = &self.tls_session;
        let stream = &self.mio_stream;

        match tls_session {
            Some(tls_session) => {
                match tls_session.lock() {
                    Ok(mut tls_session) => {
                        match stream.lock() {
                            Ok(mut stream) => {
                                //~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=
                                tls_session.flush()?;
                                stream.flush()
                                //~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=
                            }
                            Err(err) => {
                                Err(io::Error::new(ErrorKind::Other, format!("{}", err)))
                            }
                        }
                    }
                    Err(err) => {
                        Err(io::Error::new(ErrorKind::Other, format!("{}", err)))
                    }
                }
            }
            None => {
                match stream.lock() {
                    Ok(mut stream) => {
                        //~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=
                        stream.flush()
                        //~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=
                    }
                    Err(err) => {
                        Err(io::Error::new(ErrorKind::Other, format!("{}", err)))
                    }
                }
            }
        }
    }
}
