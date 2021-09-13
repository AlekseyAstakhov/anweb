use anweb::server::{Event, Server};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = ([0, 0, 0, 0], 8080).into();
    let server = Server::new(&addr)?;
    server.run(move |server_event| {
        if let Event::Connected(tcp_session) = server_event {
            tcp_session.upgrade_to_http(|http_result, http_session| {
                let request = http_result?;

                // Routing is done manually in any way.
                match request.path().as_str() {
                    "/" => {
                        http_session.response_200_html(FIRST_PAGE_HTML, &request);
                    }
                    "/second_page" => {
                        http_session.response_200_html(SECOND_PAGE_HTML, &request);
                    }
                    "/third_page" => {
                        http_session.response_200_html(THIRD_PAGE_HTML, &request);
                    }
                    _ => {
                        http_session.response_404_text("404 page not found", &request);
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
