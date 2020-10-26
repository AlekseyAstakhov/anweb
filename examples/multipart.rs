use anweb::server::{Event, Server};
use std::str::from_utf8;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = ([0, 0, 0, 0], 8080).into();

    let server = Server::new(&addr)?;
    server.run(move |server_event| {
        if let Event::Connected(client) = server_event {
            client.switch_to_http(|http_result, client| {
                let request = http_result?;
                match request.path().as_str() {
                    "/" => {
                        if request.method() == "GET" {
                            client.response_200_html(INDEX_HTML, &request);
                        }
                    }
                    "/upload" => {
                        if request.method() == "POST" {
                            let request = (*request).clone();
                            // Read all data of the request content. Let it be for now.
                            client.read_raw_content(move |content, client| {
                                // Parse content as multipart.
                                let parts = anweb::multipart::multipart(&content, &request)?;
                                let mut response_body = "".to_string();
                                for part in &parts {
                                    response_body += &format!("disposition: {:?} len: {:?} bytes\n", from_utf8(&part.disposition.raw)?, part.data.len());
                                }
                                client.response_200_text(&response_body, &request);
                                Ok(())
                            });
                        }
                    }
                    _ => {
                        client.response_404_text("404 page not found", &request);
                    }
                }

                Ok(())
            })
        }
    })?;

    Ok(())
}

const INDEX_HTML: &str = r#"
<html>
    <body>
        <h3>Upload example</h3>
        <form action="/upload" enctype="multipart/form-data" method="post">
            <input type="file" name="file" id="file" /> <br>
            <input type="submit" value="upload" />
        </form>
    </body>
</html>
"#;
