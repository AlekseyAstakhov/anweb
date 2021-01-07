use anweb::query::parse_query;
use anweb::server::{Event, Server};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = ([0, 0, 0, 0], 8080).into();

    let server = Server::new(&addr)?;
    server.run(move |server_event| {
        if let Event::Connected(client) = server_event {
            client.switch_to_http(|http_result, client| {
                let request = http_result?;
                match request.raw_path_str() {
                    "/" => {
                        if request.method() == "GET" {
                            client.response_200_html(INDEX_HTML, &request);
                        }
                    }
                    "/form" => {
                        if request.method() == "POST" {
                            if request.has_post_form() {
                                let request = (*request).clone();
                                // Read all data of the request content.
                                let mut content = vec![];
                                client.read_content(move |data, done, http_client| {
                                    content.extend_from_slice(data);
                                    if done {
                                        // Parse content data as query.
                                        let form = parse_query(&content);
                                        let response_body = format!("Form: {:?}", form);
                                        http_client.response_200_text(&response_body, &request);
                                    }

                                    Ok(())
                                });
                            } else {
                                client.response_422_text("Wrong form", request);
                            }
                        }
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
