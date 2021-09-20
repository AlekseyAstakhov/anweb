use anweb::server::{Event, Server};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = ([0, 0, 0, 0], 8080).into();
    let server = Server::new(&addr)?;
    server.run(move |server_event| {
        if let Event::Connected(tcp_session) = server_event {
            tcp_session.to_http(|http_result| {
                let request = http_result?;

                // Routing is done manually in any way.
                match request.path() {
                    "/" => {
                        request.response(200).html(FIRST_PAGE_HTML).send();
                    }
                    "/second_page" => {
                        request.response(200).html(SECOND_PAGE_HTML).send();
                    }
                    "/third_page" => {
                        request.response(200).html(THIRD_PAGE_HTML).send();
                    }
                    _ => {
                        request.response(404).html("404 page not found").send();
                    }
                }

                Ok(())
            });
        }
    })?;

    Ok(())
}

const FIRST_PAGE_HTML: &str = r#"
<html>
    <body>
        <h3>Route example</h3>
        <h4>First page</h4>
        <a href="/second_page">second page</a> <br>
        <a href="/third_page">third page</a>
    </body>
</html>
"#;

const SECOND_PAGE_HTML: &str = r#"
<html>
    <body>
        <h4>Second page</h4>
        <a href="/">first page</a> <br>
        <a href="/third_page">third page</a>
    </body>
</html>
"#;

const THIRD_PAGE_HTML: &str = r#"
<html>
    <body>
        <h4>Third page</h4>
        <a href="/">first page</a> <br>
        <a href="/second_page">second page</a>
    </body>
</html>
"#;
