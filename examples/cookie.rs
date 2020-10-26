use anweb::cookie::Cookie;
use anweb::server::{Event, Server};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = ([0, 0, 0, 0], 8080).into();

    let server = Server::new(&addr)?;
    server.run(move |server_event| {
        if let Event::Connected(client) = server_event {
            client.switch_to_http(|http_result, client| {
                let request = http_result?;
                let cookie_name = "test";

                // if cookie with "test" name are already installed on the client (browser)
                if let Some(_) = request.cookies().value(cookie_name) {
                    client.response_200_html(HTML_WHEN_COOKIE_RECEIVED, request);
                } else {
                    // if cookies are not installed, then install it
                    let cookie = Cookie {
                        name: "test",
                        value: "abc",
                        path: None,
                        domain: None,
                        http_only: true,
                        expires: None,
                        max_age: None,
                        secure: false,
                    };

                    client.response_200_html_with_cookie(HTML_WHEN_NO_COOKIE, &cookie, request);
                }

                Ok(())
            });
        }
    })?;

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
