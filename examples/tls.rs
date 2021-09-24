use anweb::redirect_server::run_redirect_server;
use anweb::server;
use anweb::server::Server;
use anweb::tls::{load_certs, load_private_key};
use rustls::{NoClientAuth, ServerConfig};
use std::sync::Arc;

/// This example demonstrates the use of https.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut tls_config = ServerConfig::new(NoClientAuth::new());
    let certs = load_certs("examples/keys/cert.pem")?;
    let private_key = load_private_key("examples/keys/key.pem")?;
    tls_config.set_single_cert_with_ocsp_and_sct(certs, private_key, vec![], vec![])?;

    let addr = ([0, 0, 0, 0], 8443).into();
    let mut server = Server::new(&addr)?;

    server.settings.tls_config = Some(Arc::new(tls_config));

    let redirect_addr = ([0, 0, 0, 0], 8080).into();
    run_redirect_server("https://127.0.0.1:8443/", redirect_addr, 4)?;

    server.run(|server_event| match server_event {
        server::Event::Incoming(tcp_session) => {
            tcp_session.to_http(|http_result| {
                let request = http_result?;
                request.response(200).text("Hello world!").send();
                Ok(())
            });
        }
        server::Event::Error(err) => {
            println!("{:?}", err);
        }
        _ => {}
    })?;

    Ok(())
}
