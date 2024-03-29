use anweb::server::{Event, Server};
use std::sync::Arc;
use std::sync::Mutex;
use std::thread::sleep;
use std::time::Duration;

// This example demonstrates the execution of operations requiring
// a long time to execute or blocking input/output.
// If during the processing of the request event to carry out lengthy operations,
// then other clients requiring a small time will wait for the end of this long operation.
// To solve this problem, you can use, for example, a thread pool.
// Attention, if you want to have support for clients with HTTP pipelining,
// you must ensure order of responses in requests order.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = Arc::new(Mutex::new(threadpool::ThreadPool::new(num_cpus::get())));

    let addr = ([0, 0, 0, 0], 8080).into();

    let server = Server::new(&addr)?;
    server.run(move |server_event| {
        if let Event::Incoming(tcp_session) = server_event {
            let pool = pool.clone();
            tcp_session.to_http(move |http_result| {
                let request = http_result?;
                match request.path() {
                    "/" => {
                        request.response(200).html(INDEX_HTML).send();
                    }
                    "/long" => {
                        let pool = pool.lock().unwrap();

                        pool.execute(move || {
                            // emitting long operation using sleep
                            sleep(Duration::from_secs(10));
                            request.response(200).html("Complete").send();
                        });
                    }
                    _ => {
                        request.response(404).text("404 page not found").send();
                    }
                }

                Ok(())
            })
        }
    })?;

    Ok(())
}

const INDEX_HTML: &str = r#"
<html>
    <body>
        <h3>Long or blocking operations example</h3>
        <form action="long" method="get">
            <button>Make long operation on server</button>
        </form>
    </body>
</html>
"#;
