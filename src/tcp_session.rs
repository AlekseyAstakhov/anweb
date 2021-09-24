use crate::http_result::{HttpResult, HttpError};
use crate::websocket::{Websocket, WebsocketResult, WebsocketError};
use rustls::Session;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::io;
use std::io::{ErrorKind, Read, Write};
use std::net::SocketAddr;
use crate::request::Request;

/// Tcp client connection to the server.
#[derive(Clone)]
pub struct TcpSession {
    /// Private data.
    pub(crate) inner: Arc<InnerTcpSession>,
}

impl TcpSession {
    /// Tsp client connection id on server in connection order.
    pub fn id(&self) -> u64 {
        self.inner.id()
    }

    /// An internet socket address, either IPv4 or IPv6.
    pub fn addr(&self) -> &SocketAddr {
        &self.inner.addr
    }

    /// Send arbitrary data to the client. Data may not be sent immediately, but in parts.
    pub fn send(&self, data: &[u8]) {
        self.inner.send(data);
    }

    /// Send arbitrary shared data to the client. Data may not be sent immediately, but in parts.
    pub fn send_arc(&self, data: &Arc<Vec<u8>>) {
        self.inner.send_arc(data);
    }

    /// To close client socket after all data sent.
    /// After closing will be generated `server::Event::Disconnected`.
    pub fn close_after_send(&self) {
        self.inner.need_close_after_sending.store(true, Ordering::SeqCst);
    }

    /// Close of client socket. After closing will be generated `server::Event::Disconnected`.
    pub fn close(&self) {
        self.inner.close();
    }

    /// Sets callback that will be called when data is read from tcp stream.
    /// Data can't be empty.
    /// Data will already decoded if tls used.
    pub fn on_data_received(&self, f: impl FnMut(&[u8]) + Send + 'static) {
        if let Ok(mut on_data_received_callback) = self.inner.on_data_received_callback.lock() {
            *on_data_received_callback = Some(Box::new(f));
        }
    }

    /// Switch to HTTP mode. Set a callback function that is called when a new HTTP request is received or error receiving it.
    pub fn to_http(&self, request_or_error_callback: impl FnMut(HttpResult) -> Result<(), Box<dyn std::error::Error>> + Send + 'static) {
        if let Ok(mut http_request_callback) = self.inner.http_request_callback.lock() {
            *http_request_callback = Some(Box::new(request_or_error_callback));
            self.inner.is_http_mode.store(true, Ordering::SeqCst);
        }
    }

    /// Need close of client socket.
    pub(crate) fn need_close(&self) -> bool {
        self.inner.need_close.load(Ordering::SeqCst)
    }

    /// Return true if client connection is using for receiving http requests and send responses.
    pub(crate) fn is_http_mode(&self) -> bool {
        self.inner.is_http_mode()
    }

    /// Helps call callback.
    pub(crate) fn call_http_callback(&self, request: HttpResult) {
        if let Ok(mut callback) = self.inner.http_request_callback.lock() {
            if let Some(callback) = &mut *callback {
                if callback(request).is_err() {
                    self.close();
                }
            }
        }
    }

    /// Helps call callback.
    pub(crate) fn call_websocket_callback(&self, frame: WebsocketResult) {
        if let Ok(mut callback) = self.inner.websocket_callback.lock() {
            if let Some(callback) = &mut *callback {
                if callback(frame, Websocket { tcp_session: self.clone() }).is_err() {
                    self.close();
                }
            }
        }
    }

    /// Called when new TCP connection.
    pub(crate) fn new(id: u64, slab_key: usize, stream: mio::net::TcpStream, addr: SocketAddr, tls_session: Option<Mutex<rustls::ServerSession>>, mio_poll: Arc<mio::Poll>, http_date_string: Arc<RwLock<String>>) -> Self {
        TcpSession {
            inner: Arc::new(InnerTcpSession {
                id,
                slab_key,
                mio_stream: Mutex::new(stream),
                addr,
                tls_session,
                on_data_received_callback: Mutex::new(None),
                http_request_callback: Mutex::new(None),
                is_http_mode: Arc::new(AtomicBool::new(false)),
                websocket_callback: Mutex::new(None),
                content_callback: Mutex::new(None),
                need_close: AtomicBool::new(false),
                surpluses_to_write: Mutex::new(Vec::new()),
                mio_poll,
                http_date_string,
                need_close_after_sending: Arc::new(AtomicBool::new(false)),
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

                self.close();
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
                            self.close();
                        } else {
                            if self.is_http_mode() {
                                self.call_http_callback(Err(HttpError::StreamError(err)));
                            } else {
                                self.call_websocket_callback(Err(WebsocketError::StreamError(err)));
                            }
                            self.close();
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
                            if self.inner.need_close_after_sending.load(Ordering::SeqCst) {
                                self.close();
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

                self.close();
            }
        } else {
            dbg!("unreachable code");
            self.close();
        }
    }
}

impl Read for TcpSession {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read_stream(buf)
    }
}

impl Write for TcpSession {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// It's use in load content callback for inform about finish of reading.
pub type ContentIsComplite = Option<Request>;

/// Private data of tcp session.
pub(crate) struct InnerTcpSession {
    /// Tcp client connection id on the server in connection order.
    id: u64,
    /// Slab key of tcp client connection on the server.
    slab_key: usize,
    /// An internet socket address, either IPv4 or IPv6.
    pub(crate) addr: SocketAddr,
    /// Stream which received from MIO event.
    pub(crate) mio_stream: Mutex<mio::net::TcpStream>,
    /// TLS session.
    tls_session: Option<Mutex<rustls::ServerSession>>,

    /// Callback function that is called when a data read from tcp socket.
    pub(crate) on_data_received_callback: Mutex<Option<Box<dyn FnMut(&[u8]) + Send>>>,

    /// Callback function that is called when a new HTTP request is received or error receiving it.
    pub(crate) http_request_callback: Mutex<Option<Box<dyn FnMut(HttpResult) -> Result<(), Box<dyn std::error::Error>> + Send>>>,
    /// Sets true when callback is set.
    pub(crate) is_http_mode: Arc<AtomicBool>,

    /// Callback function that is called when content of HTTP request is fully received or error receiving it.
    pub(crate) content_callback: Mutex<Option<(Box<dyn FnMut(&[u8]/*data part*/, ContentIsComplite) -> Result<(), Box<dyn std::error::Error>> + Send>, Option<Request>)>>,
    /// Callback function that is called when a new websocket frame is received or error receiving it.
    pub(crate) websocket_callback: Mutex<Option<Box<dyn FnMut(WebsocketResult, Websocket) -> Result<(), WebsocketError> + Send>>>,

    /// Data that was not written in one write operation and is waiting for the socket to be ready.
    surpluses_to_write: Mutex<Vec<SurplusForWrite>>,

    /// Mio poll. Need only for reregister client for readable/writable.
    mio_poll: Arc<mio::Poll>,

    /// Determines whether to close connection. Connection will be closed when all other connections with read/write readiness are processing completed.
    need_close: AtomicBool,

    /// Prepared rfc7231 string for http responses, update once per second.
    pub(crate) http_date_string: Arc<RwLock<String>>,

    /// For close the connection after the http response.
    need_close_after_sending: Arc<AtomicBool>,
}

/// Data that was not written in one write operation and is waiting for the socket to be ready.
struct SurplusForWrite {
    data: Arc<Vec<u8>>,
    write_yet_cnt: usize,
}

/// Private tcp session data.
impl InnerTcpSession {
    /// Tcp connection id on server in connection order.
    pub fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn is_http_mode(&self) -> bool {
        self.is_http_mode.load(Ordering::SeqCst)
    }

    pub fn read_stream(&self, buf: &mut [u8]) -> io::Result<usize> {
        let read_cnt = {
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

        if read_cnt == 0 {
            return Ok(0);
        }

        let call_on_data_received_callback = |data: &[u8]| {
            if let Ok(mut on_data_received_callback) = self.on_data_received_callback.lock() {
                if let Some(on_data_received_callback) = &mut *on_data_received_callback {
                    on_data_received_callback(data);
                }
            }
        };

        match &self.tls_session {
            None => {
                call_on_data_received_callback(&buf[..read_cnt]);
                Ok(read_cnt)
            },
            Some(tls_session) => {
                let read_buf: &mut dyn std::io::Read = &mut &buf[..read_cnt];
                match tls_session.lock() {
                    Ok(mut tls_session) => {
                        tls_session.read_tls(read_buf)?;

                        if let Err(err) = tls_session.process_new_packets() {
                            return Err(io::Error::new(ErrorKind::Other, err));
                        }

                        let tls_readed_cnt = tls_session.read(&mut buf[..])?;
                        while tls_session.wants_write() {
                            if let Ok(mut stream) = self.mio_stream.lock() {
                                //=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=
                                tls_session.write_tls(&mut *stream)?;
                                //=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=~=
                            }
                        }

                        if tls_readed_cnt == 0 {
                            return Err(io::Error::new(std::io::ErrorKind::WouldBlock, "operation would block"));
                        }

                        call_on_data_received_callback(&buf[..tls_readed_cnt]);

                        Ok(tls_readed_cnt)
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
                    if self.need_close_after_sending.load(Ordering::SeqCst) {
                        self.close();
                    }
                }
            }
            Err(err) => {
                if err.kind() == std::io::ErrorKind::WouldBlock {
                    self.send_later(SurplusForWrite { data: Arc::new(data.to_vec()), write_yet_cnt: 0 });
                } else if err.kind() == std::io::ErrorKind::ConnectionReset {
                    self.close();
                } else {
                    self.close();
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
                supluses.push(SurplusForWrite { data: data.clone(), write_yet_cnt: 0 });
                return;
            }
        }

        match self.write(&data) {
            Ok(cnt) => {
                if cnt < data.len() {
                    self.send_later(SurplusForWrite { data: Arc::clone(data), write_yet_cnt: cnt });
                } else {
                    // all data is written
                    if self.is_http_mode() && self.need_close_after_sending.load(Ordering::SeqCst) {
                        self.close();
                    }
                }
            }
            Err(err) => {
                if err.kind() == std::io::ErrorKind::WouldBlock {
                    self.send_later(SurplusForWrite { data: Arc::clone(data), write_yet_cnt: 0 });
                } else if err.kind() == std::io::ErrorKind::ConnectionReset {
                    self.close();
                } else {
                    self.close();
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
                        self.close();
                        return;
                    }
                }
            }
        }

        dbg!("unreachable code");
        self.close();
    }

    /// Close of client socket. After clossing will be generated `sever::Event::Disconnected`.
    pub fn close(&self) {
        self.need_close.store(true, Ordering::SeqCst);
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
