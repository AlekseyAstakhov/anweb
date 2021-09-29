use crate::server::{Error, Event, Settings, Stopper};
use crate::tcp_session::TcpSession;

use mio::net::TcpListener;
use slab::Slab;
use std::io::ErrorKind;
use std::panic;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use crate::web_session;
use crate::web_session::WebSession;

/// Single threaded TCP server designed for use as an HTTP server.
pub struct Worker {
    /// Connected clients.
    web_sessions: Slab<WebSession>,

    /// Connection counter. Used to create tcp connections identifiers. Atomic in order to identify users on several such servers.
    pub connections_counter: Arc<AtomicU64>,

    /// Server settings.
    pub settings: Settings,

    /// For stop the server.
    pub stopper: Stopper,

    mio_poll: Arc<mio::Poll>,
    events: mio::Events,
    tcp_listener: TcpListener,

    /// For update once per second.
    http_date_string: Arc<RwLock<String>>,

    /// Buffer for read from socket.
    read_buf: [u8; 1024],
}

impl Worker {
    /// Tries to start the server and returns it as a result.
    pub fn new_from_listener(tcp_listener: TcpListener, stopper: Stopper) -> Result<Worker, std::io::Error> {
        let mio_poll = mio::Poll::new()?;

        mio_poll.register(&tcp_listener, LISTENER_TOKEN, mio::Ready::readable(), mio::PollOpt::level())?;

        const POLL_EVENTS_CNT: usize = 1024;
        const CLIENTS_CAPACITY: usize = 10000;

        let http_date_string = Arc::new(RwLock::new(now_rfc7231_string()));
        start_thread_of_update_http_date_string(http_date_string.clone());

        Ok(Worker {
            web_sessions: Slab::with_capacity(CLIENTS_CAPACITY),
            connections_counter: Arc::new(AtomicU64::new(0)),
            mio_poll: Arc::new(mio_poll),
            events: mio::Events::with_capacity(POLL_EVENTS_CNT),
            tcp_listener,
            settings: Settings {
                tls_config: None,
                web_settings: web_session::Settings::default(),
            },
            stopper,
            http_date_string,
            read_buf: [0; 1024],
        })
    }

    /// Poll mio, process MIO events, read data processing (parse HTTP, etc.), generate events and do some based on user response to event.
    pub fn poll(&mut self, timeout: Option<Duration>, event_callback: &mut (dyn FnMut(Event))) {
        self.remove_if_need_close(event_callback);

        let poll_res = self.mio_poll.poll(&mut self.events, timeout);
        if let Err(err) = poll_res {
            event_callback(Event::Error(Error::PollError(err)));
            return;
        }

        self.process_mio_events(event_callback);
    }

    /// Run server. See 'poll'.
    pub fn run(&mut self, event_callback: &mut (dyn FnMut(Event))) {
        loop {
            if self.stopper.need_stop() {
                break;
            }

            self.poll(None, event_callback);
        }
    }

    /// Process MIO events. Register new tcp connections.
    fn process_mio_events(&mut self, event_callback: &mut (dyn FnMut(Event))) {
        for event in self.events.iter() {
            match event.token() {
                LISTENER_TOKEN => {
                    while let Ok((stream, addr)) = self.tcp_listener.accept() {
                        let session_id = self.connections_counter.fetch_add(1, Ordering::SeqCst);
                        let slab_key = self.web_sessions.vacant_entry().key();

                        let rustls_session = match &self.settings.tls_config {
                            Some(tls_config) => Some(Mutex::new(rustls::ServerSession::new(&tls_config))),
                            None => None,
                        };

                        let tcp_session = TcpSession::new(session_id, slab_key, stream, addr, rustls_session, self.mio_poll.clone(), self.http_date_string.clone());
                        let web_session = WebSession::new(tcp_session.clone());

                        event_callback(Event::Incoming(tcp_session.clone()));

                        if tcp_session.need_close() {
                            continue;
                        }

                        let register_result;
                        match tcp_session.inner.mio_stream.lock() {
                            Ok(stream) => {
                                register_result = self.mio_poll.register(&*stream, mio::Token(slab_key), mio::Ready::readable(), mio::PollOpt::level());
                            }
                            Err(err) => {
                                let err = std::io::Error::new(ErrorKind::Other, format!("{}", err));
                                event_callback(Event::Error(Error::RegisterError(err)));
                                event_callback(Event::Closed(session_id));
                                continue;
                            }
                        }

                        match register_result {
                            Ok(()) => {
                                self.web_sessions.insert(web_session);
                            }
                            Err(err) => {
                                event_callback(Event::Error(Error::RegisterError(err)));
                                event_callback(Event::Closed(session_id));
                            }
                        }
                    }
                }
                mio::Token(token_id) => {
                    let mut need_remove = None;

                    if event.readiness().is_readable() {
                        // there is a possibility of receiving events on a already removed session if library user cloned stream and not deleted yet
                        if let Some(session) = self.web_sessions.get_mut(token_id) {
                            let session_settings = &self.settings.web_settings;

                            let read_buf = &mut self.read_buf[..];
                            let catch_result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
                                session.read_stream(session_settings, read_buf);
                            }));

                            if catch_result.is_err() {
                                need_remove = Some(session.tcp_session.id());
                                event_callback(Event::Error(Error::Panicked(session.tcp_session.id())));
                            } else if session.tcp_session.need_close() {
                                need_remove = Some(session.tcp_session.id());
                            }
                        }
                    }

                    if event.readiness().is_writable() {
                        if let Some(session) = self.web_sessions.get_mut(token_id) {
                            session.tcp_session.send_yet();

                            if session.tcp_session.need_close() {
                                need_remove = Some(session.tcp_session.id());
                            }
                        }
                    }

                    if let Some(session_id) = need_remove {
                        self.web_sessions.remove(token_id);
                        event_callback(Event::Closed(session_id));
                    }
                }
            }
        }
    }

    /// Removes sessions that no need.
    fn remove_if_need_close(&mut self, event_callback: &mut (dyn FnMut(Event))) {
        self.web_sessions.retain(|_, web_session| {
            if web_session.tcp_session.need_close() {
                event_callback(Event::Closed(web_session.tcp_session.id()));
                return false;
            }

            true
        });
    }
}

/// MIO key of server listener.
const LISTENER_TOKEN: mio::Token = mio::Token(usize::MAX - 1);

/// Returns string date in 7231 format.
pub fn now_rfc7231_string() -> String {
    chrono::Utc::now().to_rfc2822().replace("+0000", "GMT")
}

/// Update http date header once per second in own thread.
fn start_thread_of_update_http_date_string(http_date_string: Arc<RwLock<String>>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(1000));
        if let Ok(mut http_date_string) = http_date_string.write() {
            *http_date_string = now_rfc7231_string();
        }
    });
}
