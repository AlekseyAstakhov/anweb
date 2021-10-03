use anweb::cookie::Cookie;
use anweb::server::{Event, Server};
use anweb::request::Request;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = ([0, 0, 0, 0], 8080).into();

    let server = Server::new(&addr)?;
    server.run(move |server_event| {
        if let Event::Incoming(tcp_session) = server_event {
            tcp_session.to_http(move |http_result| {
                on_request(http_result?)
            });
        }
    })?;

    Ok(())
}

fn on_request(request: Request) -> Result<(), Box<dyn std::error::Error>> {
    let cookie_name = "test";

    // if cookie with "test" name are already installed on the client (browser)
    if let Some(_) = request.cookies().iter().find(|cookie| cookie.name == cookie_name) {
        request.response(200).html(HTML_WHEN_COOKIE_RECEIVED).send();
    } else {
        let cookie = Cookie {
            name: "test",
            value: "abc",
            path: None,
            domain: None,
            http_only: true,
            expires: None,
            max_age: None,
            secure: false,
        }.to_string();

        // if cookies are not installed, then install it
        request.response(200)
            .cookies(&cookie)
            .html(HTML_WHEN_NO_COOKIE)
            .send();
    }

    Ok(())
}

const HTML_WHEN_NO_COOKIE: &str = r#"
<html>
    <body>
        <h3>Cookie example</h3>
        <p>Set-Cookie request was sent, update this page!</p>
    </body>
</html>
"#;

const HTML_WHEN_COOKIE_RECEIVED: &str = r#"
<html>
    <body>
        <p>If you see this text then cookie was received on server.</p>
    </body>
</html>
"#;
