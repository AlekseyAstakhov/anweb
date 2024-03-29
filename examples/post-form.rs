use anweb::server::{Event, Server};
use anweb::request::Request;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = ([0, 0, 0, 0], 8080).into();

    let server = Server::new(&addr)?;
    server.run(move |server_event| {
        if let Event::Incoming(tcp_session) = server_event {
            tcp_session.to_http(|http_result| {
                on_request(http_result?)
            });
        }
    })?;

    Ok(())
}

fn on_request(request: Request) -> Result<(), Box<dyn std::error::Error>> {
    let path = request.path().clone();
    match path {
        "/" => {
            if request.method() == "GET" {
                request.response(200).html(INDEX_HTML).send();
                return Ok(());
            }
        }
        "/form" => {
            if request.method() == "POST" {
                request.form(|form, request| {
                    let response_body = format!("Form: {:?}", form);
                    request.response(200).text(&response_body).send();
                    Ok(())
                });
                return Ok(());
            }
        }
        _ => {
        }
    }

    request.response(404).text("404 page not found").send();

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
