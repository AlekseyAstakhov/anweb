use anweb::server::{Event, Server};
use anweb::static_files::StaticFiles;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The data of all files in the dir will be loaded into RAM
    // and stored in a prepared, compressed form.
    // All changes to the directory will update the content in RAM.
    // This also handles the browser cache.
    // For advanced settings see: 'static_files::Builder'
    let static_files = StaticFiles::new(current_src_dir_path());

    let addr = ([0, 0, 0, 0], 8080).into();

    let server = Server::new(&addr)?;
    server.run(move |server_event| {
        if let Event::Connected(client) = server_event {
            let static_files = static_files.clone();
            client.switch_to_http(move |http_result, client| {
                let request = http_result?;
                match request.path().as_str() {
                    "/" => {
                        client.response_200_html(&index_page_html(static_files.files()), request);
                    }
                    path => {
                        // File data or cache confirmation will be sent with response.
                        static_files.response(path, request, &client)?;
                    }
                }

                Ok(())
            })
        }
    })?;

    Ok(())
}

/// Response body with list of links to files in this source directory.
fn index_page_html(file_names: Vec<String>) -> String {
    let mut body = "<html>\n<body>\n<h3>Static files example</h3>\n".to_string();

    for file_name in file_names {
        body += &format!("<a href=\"{}\">{}</a> <br>\n", &file_name, &file_name);
    }

    body += "</body>\n</html>\n";

    body
}

/// Directory path of current source code file.
fn current_src_dir_path() -> &'static str {
    let src_file_path = file!();
    let index_of_file = src_file_path.rfind('/').unwrap_or(0);
    &src_file_path[..index_of_file]
}
