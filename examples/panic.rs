use anweb::server::{Event, Server};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = ([0, 0, 0, 0], 8080).into();
    let server = Server::new(&addr)?;
    server.run(move |server_event| {
        if let Event::Connected(client) = server_event {
            client.switch_to_http(|http_result, client| {
                let request = http_result?;
                match request.path().as_str() {
                    "/" => {
                        client.response_200_html(INDEX_HTML, &request);
                    }
                    "/panic" => {
                        // If there is a panic in the request processing code, the client connection
                        // will be closed and the associated resources will be cleaned up.
                        // After that there will be a server event Event::Disconnected.
                        panic!("panic test");
                    }
                    _ => {
                        client.response_404_text("404 page not found", &request);
                    }
                }

                Ok(())
            });
        }
    })?;

    Ok(())
}

const INDEX_HTML: &str = r#"
<html>
    <body>
        <h3>Panic example</h3>
        <form action="panic" method="get">
            <button>Make panic on server</button>
        </form>
    </body>
</html>
"#;
