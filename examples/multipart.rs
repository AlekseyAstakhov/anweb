use anweb::server::{Event, Server};
use anweb::multipart::{MultipartParser, MultipartParserEvent};
use std::str::from_utf8;
use anweb::request::Request;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = ([0, 0, 0, 0], 8080).into();

    let server = Server::new(&addr)?;
    server.run(move |server_event| {
        if let Event::Connected(tcp_session) = server_event {
            tcp_session.to_http(|http_result| {
                on_request(http_result?)
            })
        }
    })?;

    Ok(())
}

fn on_request(request: Request) -> Result<(), Box<dyn std::error::Error>> {
    match request.path() {
        "/" => {
            if request.method() == "GET" {
                request.response(200).html(INDEX_HTML).send();
            }
        }
        "/form" => {
            if request.method() == "POST" {
                let mut multipart = MultipartParser::new(&request)?;
                let mut response_body = String::new();
                let cloned_request = request.clone();
                request.read_content(move |data, done| {
                    multipart.push(data, |ev| {
                        match ev {
                            MultipartParserEvent::Disposition(disposition) => {
                                response_body += &format!("disposition: {:?}\n", from_utf8(&disposition.raw).unwrap());
                            },
                            MultipartParserEvent::Data { data_part: _, end: _ } => {
                            },
                            MultipartParserEvent::Finished => {
                            },
                        }
                    })?;

                    if done {
                        cloned_request.response(200).text(&response_body).send();
                    }

                    Ok(())
                });
            }
        }
        _ => {
            request.response(404).text("404 page not found").send();
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
