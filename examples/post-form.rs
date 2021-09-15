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
    match request.raw_path_str() {
        "/" => {
            if request.method() == "GET" {
                http_session.response_200_html(INDEX_HTML, &request);
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
                            http_session.response_200_text(&response_body, &request);
                        }

                        Ok(())
                    });
                } else {
                    http_session.response_422_text("Wrong form", request);
                }
            }
        }
        _ => {
            http_session.response_404_text("404 page not found", &request);
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
