use crate::tcp_client;
use crate::tcp_session::TcpSession;
use crate::worker::Worker;

use mio::net::TcpListener;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Arc;
use std::thread::JoinHandle;

/// Server event.
pub enum Event {
    /// Server has started (listening started).
    Started,
    /// New TCP connection has been established.
    Connected(TcpSession),
    /// TCP connection was closed. This can be caused either by the server’s initiative when the connection cannot be served, or by forced closure at the initiative of the library user.
    Disconnected(u64 /*id*/),
    /// Server error.
    Error(Error),
}

/// HTTP server errors.
#[derive(Debug)]
pub enum Error {
    /// MIO poll error.
    PollError(std::io::Error),
    /// MIO register error.
    RegisterError(std::io::Error),
    /// If panicked when processing client incoming data including library user code. Client will be disconnected.
    Panicked(u64 /*client id*/),
    /// When worker was not created (create mio poll or register listener error).
    WorkerNotCreated(std::io::Error),
    /// Worker panicked with cause of panic.
    WorkerPanicked(Box<dyn std::any::Any>),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for Error {}

#[derive(Clone)]
/// Server settings.
pub struct Settings {
    /// Configuration of TLS (rustls).
    pub tls_config: Option<Arc<rustls::ServerConfig>>,
    // Client settings: HTTP parser and websocket settings.
    pub clients_settings: tcp_client::Settings,
}

/// Multithreaded TCP server designed for use as an HTTP server.
pub struct Server {
    /// Worker thread handles for this server.
    workers: Vec<JoinHandle<()>>,
    /// MOI tcp listener.
    tcp_listener: TcpListener,
    /// Number of worker thread. Defaults to the number of available CPUs of the current system. You can change this value before starting server (before call 'run').
    pub num_threads: usize,
    /// Settings of this server such as tls, http parsing, websockets and etc.
    pub settings: Settings,

    /// For stop the server.
    pub stopper: Arc<AtomicBool>,
}

impl Server {
    /// Constructs new HTTP server with default settings. Create new MIO listener. The created server is not running, to start, you need to call 'run' method.
    pub fn new(addr: &SocketAddr) -> Result<Server, std::io::Error> {
        let tcp_listener = TcpListener::bind(&addr)?;
        Ok(Self::new_from_listener(tcp_listener))
    }

    /// Constructs new HTTP server with default settings from existing MIO tcp listener. The created server is not running, to start, you need to call 'run' method.
    pub fn new_from_listener(tcp_listener: TcpListener) -> Self {
        Server {
            workers: vec![],
            tcp_listener,
            num_threads: num_cpus::get(),
            settings: Settings {
                tls_config: None,
                clients_settings: tcp_client::Settings::default(),
            },
            stopper: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Starts the server entering an infinite loop.
    ///
    /// # Arguments
    ///
    /// * `event_callback` - A server event callback function.
    /// ```
    pub fn run(mut self, event_callback: impl Fn(Event) + Send + Clone + 'static) -> Result<(), std::io::Error> {
        self.workers = Vec::with_capacity(self.num_threads);

        let connections_counter = Arc::new(AtomicU64::new(0));

        for _ in 0..self.num_threads {
            let cloned_tcp_listener = self.tcp_listener.try_clone()?;
            let connections_counter = connections_counter.clone();
            let event_callback = event_callback.clone();

            let settings = self.settings.clone();

            match Worker::new_from_listener(cloned_tcp_listener, self.stopper.clone()) {
                Ok(mut worker) => {
                    self.workers.push(std::thread::spawn(move || {
                        worker.connections_counter = connections_counter;
                        worker.settings = settings;
                        worker.run(&mut |event| event_callback(event));
                    }));
                }
                Err(err) => {
                    event_callback(Event::Error(Error::WorkerNotCreated(err)));
                }
            }
        }

        event_callback(Event::Started);

        for w in self.workers {
            w.join().unwrap_or_else(|err| {
                event_callback(Event::Error(Error::WorkerPanicked(err)));
            });
        }

        Ok(())
    }
}
