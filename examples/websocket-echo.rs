use anweb::server;
use anweb::server::Server;
use anweb::websocket::frame;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = ([0, 0, 0, 0], 8080).into();
    let mut server = Server::new(&addr)?;

    server.settings.clients_settings.websocket_payload_limit = 10000;

    server.run(|server_event| {
        if let server::Event::Connected(tcp_session) = server_event {
            tcp_session.to_http(|http_result| {
                let mut request = http_result?;
                match request.path() {
                    "/" => {
                        request.response(200).html(INDEX_HTML).send();
                    }
                    "/ws" => {
                        // Try process websocket handshake request and switch connection
                        // to websocket mode, it will no longer process http requests.
                        request.accept_websocket(vec![], |websocket_result, mut websocket_session| {
                            // This callback will be called if a new frame arrives from the client
                            // or an error occurs.
                            let received_frame = websocket_result?;
                            websocket_session.send(&frame(received_frame.opcode(), received_frame.payload()));
                            // Need return Ok(()) from this callback.
                            // If you return any error then the tcp client connection will be closed.
                            Ok(())
                        })?;
                    }
                    _ => {
                        request.response(404).text("404 page not found").send();
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
        <script>
            var socket = new WebSocket("ws://127.0.0.1:8080/ws", "echo");

            function sendToServer(data) {
                socket.send(data);
            }

            socket.onmessage = function(event) {
               document.getElementById('fromServer').innerHTML += event.data + '<br>';
            }
        </script>

    	<h3>Websocket echo example</h3>
        <form onsubmit="sendToServer(document.getElementById('text').value); return false;">
            <input type="text" id="text" /> <br>
            <button type="submit">Send</button> <br>
        </form>

        <p id="fromServer"/> </p>
    </body>
</html>
"#;
