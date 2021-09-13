use anweb::server::{Event, Server};
use anweb::multipart::{MultipartParser, MultipartParserEvent};
use std::str::from_utf8;
use anweb::http_client::{HttpResult, HttpClient};
use anweb::request::Request;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = ([0, 0, 0, 0], 8080).into();

    let server = Server::new(&addr)?;
    server.run(move |server_event| {
        if let Event::Connected(client) = server_event {
            client.switch_to_http(|http_result, client| {
                on_request(http_result?, client)
            })
        }
    })?;

    Ok(())
}

fn on_request(request: &Request, client: HttpClient) -> Result<(), Box<dyn std::error::Error>> {
    match request.path().as_str() {
        "/" => {
            if request.method() == "GET" {
                client.response_200_html(INDEX_HTML, &request);
            }
        }
        "/form" => {
            if request.method() == "POST" {
                let request = (*request).clone();
                let mut multipart = MultipartParser::new(&request)?;
                let mut response_body = "".to_string();
                client.read_content(move |data, done, client| {
                    multipart.push(data, |ev| {
                        match ev {
                            MultipartParserEvent::Disposition(disposition) => {
                                response_body += &format!("disposition: {:?}\n", from_utf8(&disposition.raw).unwrap());
                            },
                            MultipartParserEvent::Data { data: _, end: _ } => {
                            },
                        }
                    })?;

                    if done {
                        client.response_200_text(&response_body, &request);
                    }

                    Ok(())
                });
            }
        }
        _ => {
            client.response_404_text("404 page not found", &request);
        }
    }

    Ok(())
}

const INDEX_HTML: &str = r#"
<html>
    <body>
        <h3>Multipart example</h3>
        <form action="/form" enctype="multipart/form-data" method="post">
            <input type="file" name="file" id="file"/> <br>
            <input type="text" name="text1" value="some text"/> <br>
            <input type="submit" value="send"/>
        </form>
    </body>
</html>
"#;
