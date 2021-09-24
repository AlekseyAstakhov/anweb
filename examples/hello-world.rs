use anweb::server::{Event, Server};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = ([0, 0, 0, 0], 8080).into();
    let server = Server::new(&addr)?;
    // Calling the 'run' function will result in an endless loop of waiting for activity such
    // as clients connecting or read/write ready.
    server.run(move |server_event| {
        if let Event::Incoming(tcp_session) = server_event {
            // Start using TCP connection for http
            tcp_session.to_http(|request| {
                // This callback receives a http requests
                // or errors such as working with a socket, parsing of request, etc.

                // Send response
                request?.response(200).text("Hello world!").send();

                // Need return Ok(()) from this callback if all ok.
                // If return any error that received into this callback then default actions
                // for that error will be taken.
                // If return any other error then the session will be closed.
                Ok(())
            });
        }
    })?;

    Ok(())
}
