use anweb::query::parse_query;
use anweb::server::{Event, Server};
use anweb::request::Request;
use anweb::http_session::HttpSession;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = ([0, 0, 0, 0], 8080).into();

    let server = Server::new(&addr)?;
    server.run(move |server_event| {
        if let Event::Connected(tcp_session) = server_event {
            tcp_session.upgrade_to_http(|http_result, http_session| {
                on_request(http_result?, &http_session)
            });
        }
    })?;

    Ok(())
}

fn on_request(request: &Request, http_session: &HttpSession) -> Result<(), Box<dyn std::error::Error>> {
    match request.path() {
        "/" => {
            if request.method() == "GET" {
                http_session.response(200).html(INDEX_HTML).send(&request);
            }
        }
        "/form" => {
            if request.method() == "POST" {
                if request.has_post_form() {
                    let request = (*request).clone();
                    // Read content of the request.
                    let mut content = vec![];
                    http_session.read_content(move |data, done, http_session| {
                        // Collect content chunks.
                        content.extend_from_slice(data);
                        // When all chunks received
                        if done {
                            // Parse content data as query.
                            let form = parse_query(&content);
                            let response_body = format!("Form: {:?}", form);
                            http_session.response(200).text(&response_body).send(&request);
                        }

                        Ok(())
                    });
                } else {
                    http_session.response(422).text("Wrong form").send(&request);
                }
            }
        }
        _ => {
            http_session.response(404).text("404 page not found").send(&request);
        }
    }

    Ok(())
}

const INDEX_HTML: &str = r#"
<html>
    <body>
        <h3>Post form example</h3>
        <form action="form" method="post">
            <input type="text" name="first" />
            <br>
            <input type="text" name="second" />
            <br>
            <input type="submit" />
        </form>
    </body>
</html>
"#;
