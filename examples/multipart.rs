use anweb::server::{Event, Server};
use anweb::multipart::{MultipartParser, MultipartParserEvent};
use std::str::from_utf8;
use anweb::http_session::HttpSession;
use anweb::request::Request;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = ([0, 0, 0, 0], 8080).into();

    let server = Server::new(&addr)?;
    server.run(move |server_event| {
        if let Event::Connected(tcp_session) = server_event {
            tcp_session.upgrade_to_http(|http_result, http_session| {
                on_request(http_result?, http_session)
            })
        }
    })?;

    Ok(())
}

fn on_request(request: &Request, http_session: HttpSession) -> Result<(), Box<dyn std::error::Error>> {
    match request.path() {
        "/" => {
            if request.method() == "GET" {
                http_session.response(200).html(INDEX_HTML).send(&request);
            }
        }
        "/form" => {
            if request.method() == "POST" {
                let request = (*request).clone();
                let mut multipart = MultipartParser::new(&request)?;
                let mut response_body = String::new();
                    http_session.read_content(move |data, done, http_session| {
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
                        http_session.response(200).text(&response_body).send(&request);
                    }

                    Ok(())
                });
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
        <h3>Multipart example</h3>
        <form action="/form" enctype="multipart/form-data" method="post">
            <input type="file" name="file" id="file"/> <br>
            <input type="text" name="text1" value="some text"/> <br>
            <input type="submit" value="send"/>
        </form>
    </body>
</html>
"#;
