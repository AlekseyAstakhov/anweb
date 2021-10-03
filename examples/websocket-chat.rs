use anweb::redirect_server::run_redirect_server;
use anweb::server;
use anweb::server::Server;
use anweb::tls::{load_certs, load_private_key};
use anweb::websocket::{frame, Frame, TEXT_OPCODE, Websocket};
use rustls::{NoClientAuth, ServerConfig};
use std::collections::btree_map::BTreeMap;
use std::str::from_utf8;
use std::sync::{Arc, Mutex, RwLock};
use anweb::request::Request;

struct Chat {
    users: RwLock<BTreeMap<u64 /*tcp session id*/, Websocket>>,
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

    server.settings.web_settings.websocket_payload_limit = 1000;

    server.run(move |server_event| {
        match server_event {
            server::Event::Incoming(tcp_session) => {
                let chat = chat.clone();
                tcp_session.to_http(move |http_result| {
                    on_request(http_result?, &chat)
                });
            }
            server::Event::Closed(sesion_id) => {
                if let Ok(mut users) = chat.users.write() {
                    users.remove(&sesion_id);
                }
            }
            _ => (),
        }
    })?;

    Ok(())
}

fn on_request(request: Request, chat: &Arc<Chat>) -> Result<(), Box<dyn std::error::Error>> {
    match request.path() {
        "/" => {
            request.response(200).html(INDEX_HTML).send();
        }
        "/ws" => {
            if let Ok(messages) = chat.messages.lock() {
                // Prepares full current chat that will send with response to handshake request.
                let mut full_chat_frames = vec![];
                for msg in messages.iter() {
                    full_chat_frames.extend(frame(TEXT_OPCODE, msg.as_bytes()));
                }

                let cloned_chat = chat.clone();
                let websocket = request.accept_websocket(Some(&full_chat_frames))?;
                websocket.on_frame(move |received_frame, _| {
                    on_websocket_frame(received_frame?, &cloned_chat);
                    Ok(())
                });

                let mut users = chat.users.write().unwrap();
                users.insert(websocket.tcp_session().id(), websocket.clone());
            }
        }
        _ => {
            request.response(404).text("404 page not found").send();
        }
    }

    Ok(())
}

fn on_websocket_frame(received_frame: &Frame, chat: &Chat) {
    if received_frame.is_text() {
        if let Ok(text) = from_utf8(received_frame.payload()) {
            let mut messages = chat.messages.lock().unwrap();
            messages.push(text.to_string());
            let users = chat.users.read().unwrap();
            for (_, websocket) in users.iter() {
                let websocket = websocket.clone();
                websocket.send(TEXT_OPCODE, text.as_bytes());
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
