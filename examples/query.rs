use anweb::http_session::HttpSession;
use anweb::request::Request;
use anweb::server::{Event, Server};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = ([0, 0, 0, 0], 8080).into();
    let server = Server::new(&addr)?;
    server.run(move |server_event| {
        if let Event::Connected(tcp_session) = server_event {
            tcp_session.upgrade_to_http(|http_result, http_session| {
                let request = http_result?;
                match request.path() {
                    "/" => {
                        http_session.response(200).html(INDEX_HTML).send(&request);
                    }
                    "/query" => {
                        on_query(request, &http_session)?;
                    }
                    _ => {
                        http_session.response(404).text("404 page not found").send(&request);
                    }
                }

                Ok(())
            });
        }
    })?;

    Ok(())
}

/// If "/query" path.
fn on_query(request: &Request, http_session: &HttpSession) -> Result<(), std::io::Error> {
    // Parse query.
    let query = request.query();
    if let Some(first_value) = query.value("first") {
        // get second value by index, if no value result will by empty
        let second_value = query.value_at(1).unwrap_or("".to_string());
        let response_body = format!("Query: first = {:?}, second = {:?}", first_value, second_value);
        http_session.response(200).html(&response_body).send(&request);
    } else {
        http_session.response(422).text("Wrong query").send(&request);
    }

    Ok(())
}

const INDEX_HTML: &str = r#"
<html>
    <body>
    	<h3>Query example</h3>
        <form action="query" method="get">
            <input type="text" name="first" />
            <input type="text" name="second" />
            <input type="submit" />
        </form>
    </body>
</html>
"#;
