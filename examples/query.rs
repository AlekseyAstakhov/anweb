use anweb::request::Request;
use anweb::server::{Event, Server};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = ([0, 0, 0, 0], 8080).into();
    let server = Server::new(&addr)?;
    server.run(move |server_event| {
        if let Event::Connected(tcp_session) = server_event {
            tcp_session.to_http(|http_result| {
                let request = http_result?;
                match request.path() {
                    "/" => {
                        request.response(200).html(INDEX_HTML).send();
                    }
                    "/query" => {
                        on_query(request)?;
                    }
                    _ => {
                        request.response(404).text("404 page not found").send();
                    }
                }

                Ok(())
            });
        }
    })?;

    Ok(())
}

/// If "/query" path.
fn on_query(request: Request) -> Result<(), std::io::Error> {
    // Parse query.
    let query = request.query();
    if let Some(first_value) = query.value("first") {
        // get second value by index, if no value result will by empty
        let second_value = query.value_at(1).unwrap_or("".to_string());
        let response_body = format!("Query: first = {:?}, second = {:?}", first_value, second_value);
        request.response(200).html(&response_body).send();
    } else {
        request.response(422).text("Wrong query").send();
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
