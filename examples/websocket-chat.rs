use anweb::redirect_server::run_redirect_server;
use anweb::server;
use anweb::server::Server;
use anweb::tls::{load_certs, load_private_key};
use anweb::websocket::{frame, handshake_response, ParsedFrame, TEXT_OPCODE};
use anweb::websocket_session::WebsocketSession;
use rustls::{NoClientAuth, ServerConfig};
use std::collections::btree_map::BTreeMap;
use std::str::from_utf8;
use std::sync::{Arc, Mutex, RwLock};

struct Chat {
    users: RwLock<BTreeMap<u64 /*id*/, WebsocketSession>>, // Cloned client tcp stream by id.
    messages: Mutex<Vec<String>>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chat = Arc::new(Chat {
        users: RwLock::new(BTreeMap::new()),
        messages: Mutex::new(Vec::new()),
    });

    let redirect_addr = ([0, 0, 0, 0], 8080).into();
    run_redirect_server("https://127.0.0.1:8443/", redirect_addr, 1)?;

    let addr = ([0, 0, 0, 0], 8443).into();
    let mut server = Server::new(&addr)?;

    let mut tls_config = ServerConfig::new(NoClientAuth::new());
    let certs = load_certs("examples/keys/cert.pem")?;
    let private_key = load_private_key("examples/keys/key.pem")?;
    tls_config.set_single_cert_with_ocsp_and_sct(certs, private_key, vec![], vec![])?;

    server.settings.tls_config = Some(Arc::new(tls_config));

    server.settings.clients_settings.websocket_payload_limit = 1000;

    server.run(move |server_event| {
        match server_event {
            server::Event::Connected(tcp_session) => {
                let chat = chat.clone();
                tcp_session.upgrade_to_http(move |http_result, mut http_session| {
                    let request = http_result?;
                    match request.path() {
                        "/" => {
                            http_session.response(200).html(INDEX_HTML).send(&request);
                        }
                        "/ws" => {
                            let mut handshake_response = handshake_response(&request)?;
                            // give current chat
                            let messages = chat.messages.lock().unwrap();
                            for msg in messages.iter() {
                                handshake_response.extend(frame(TEXT_OPCODE, msg.as_bytes()));
                            }

                            let cloned_chat = chat.clone();
                            let websocket = http_session.accept_websocket(&request, handshake_response, move |received_frame, _| {
                                let received_frame = received_frame?;
                                on_websocket_frame(&received_frame, &cloned_chat);
                                Ok(())
                            })?;

                            let mut users = chat.users.write().unwrap();
                            users.insert(websocket.id(), websocket.clone());
                        }
                        _ => {
                            http_session.response(404).text("404 page not found").send(&request);
                        }
                    }

                    Ok(())
                });
            }
            server::Event::Disconnected(client_id) => {
                if let Ok(mut users) = chat.users.write() {
                    users.remove(&client_id);
                }
            }
            _ => (),
        }
    })?;

    Ok(())
}

fn on_websocket_frame(received_frame: &ParsedFrame, chat: &Chat) {
    if received_frame.is_text() {
        if let Ok(text) = from_utf8(received_frame.payload()) {
            let mut messages = chat.messages.lock().unwrap();
            messages.push(text.to_string());
            let users = chat.users.read().unwrap();
            for (_, websocket_session) in users.iter() {
                let mut websocket_session = websocket_session.clone();
                websocket_session.send(&frame(TEXT_OPCODE, text.as_bytes()));
            }
        }
    }
}

const INDEX_HTML: &str = r#"
<html>
    <body>
        <script>
            var socket = new WebSocket("wss://127.0.0.1:8443/ws", "chat");
            
            function sendToServer(data) {
                socket.send(data);
            }

            socket.onmessage = function(event) {
               document.getElementById('fromServer').innerHTML += event.data + '<br>';
            }
        </script>

    	<h3>Websocket chat example</h3>
        <form onsubmit="sendToServer(document.getElementById('text').value); return false;">
            <input type="text" id="text" /> <br>
            <button type="submit">Send</button> <br>
        </form>
        
        <p id="fromServer"/> </p>
    </body>
</html>
"#;
