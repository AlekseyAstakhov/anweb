use anweb::server::{Event, Server};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = ([0, 0, 0, 0], 8080).into();
    let server = Server::new(&addr)?;
    // Calling the 'run' function will result in an endless loop of waiting for activity such
    // as clients connecting or read/write ready.
    server.run(move |server_event| {
        if let Event::Connected(tcp_session) = server_event {
            tcp_session.upgrade_to_http(|request, http_session| {
                // Send "Hello world!" response to any request.
                http_session.response_200_text("Hello world!", request?);
                // Need return Ok(()) from this callback.
                // If you return any error then the tcp client connection will be closed.
                Ok(())
            });
        }
    })?;

    Ok(())
}
