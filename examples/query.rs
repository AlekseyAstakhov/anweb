use anweb::http_client::HttpClient;
use anweb::request::Request;
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
                        client.response_200_html(INDEX_HTML, request);
                    }
                    "/query" => {
                        on_query(request, &client)?;
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

/// If "/query" path.
fn on_query(request: &Request, http_client: &HttpClient) -> Result<(), std::io::Error> {
    // Parse query.
    let query = request.query();
    if let Some(first_value) = query.value("first") {
        // get second value by index, if no value result will by empty
        let second_value = query.value_at(1).unwrap_or("".to_string());
        let response_body = format!("Query: first = {:?}, second = {:?}", first_value, second_value);
        http_client.response_200_text(&response_body, request);
    } else {
        http_client.response_422_text("Wrong query", request);
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
