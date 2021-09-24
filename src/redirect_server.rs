use crate::server;
use crate::worker::Worker;
use mio::net::TcpListener;
use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread::spawn;

/// Run http server in own thread. Send redirect response to any request.
pub fn run_redirect_server(path: &'static str, server_addr: SocketAddr, num_thread: usize) -> Result<(), std::io::Error> {
    let tcp_listener = TcpListener::bind(&server_addr)?;

    let stopper = Arc::new(AtomicBool::new(false));

    for _ in 0..num_thread {
        let cloned_tcp_listener = tcp_listener.try_clone()?;
        let path = path.to_string();

        let mut server = Worker::new_from_listener(cloned_tcp_listener, stopper.clone())?;

        spawn(move || {
            server.run(&mut |server_event| {
                if let server::Event::Incoming(tcp_session) = server_event {
                    let path = path.clone();
                    tcp_session.to_http(move |http_request| {
                        http_request?.response(303).location(&path).close().send();
                        Ok(())
                    });
                }
            });
        });
    }

    Ok(())
}
