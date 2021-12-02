# anweb
Mini web backend framework in Rust.
Asynchronous, lightweight, high performance and fast compile.
Built on MIO crate.

Currently in draft development.

### Goals
To implement minimal sufficient functionality for developing secure, high-performance web servers.

### Example
```rust
    use anweb::server::{Event, Server};

    fn main() -> Result<(), Box<dyn std::error::Error>> {
        let addr = ([0, 0, 0, 0], 8080).into();
        let server = Server::new(&addr)?;

        server.run(|server_event| {
            if let Event::Incoming(tcp_session) = server_event {
                tcp_session.to_http(|request| {
                    request?.response(200).text("Hello world!").send();
                    Ok(())
                });
            }
        })?;

        Ok(())
    }
```

### Safety
100% safe rust code in this crate and has minimal dependencies on third-party crates with unsafe code.

### Perfomance
On Linux it's very fast.

On Ubuntu 21.04 with AMD® A8-3870 output of 

wrk -t4 -c400 -d30s http://127.0.0.1:8080/

for "Hello world!" example:


abweb (v0.0.1):

    Running 30s test @ http://127.0.0.1:8080/
      4 threads and 400 connections
      Thread Stats   Avg      Stdev     Max   +/- Stdev
        Latency     2.37ms    2.39ms  27.52ms   88.59%
        Req/Sec    44.96k    13.45k   76.92k    58.19%
      5370013 requests in 30.10s, 783.55MB read
    Requests/sec: 178403.86
    Transfer/sec:     26.03MB

actix-web (v3.3.2):

    Running 30s test @ http://127.0.0.1:8080/
      4 threads and 400 connections
      Thread Stats   Avg      Stdev     Max   +/- Stdev
        Latency     2.42ms  674.95us  22.80ms   94.41%
        Req/Sec    34.57k     1.06k   41.18k    88.21%
      4132883 requests in 30.09s, 508.44MB read
    Requests/sec: 137353.13
    Transfer/sec:     16.90MB

go (v1.16.2):

    Running 30s test @ http://127.0.0.1:8080/
      4 threads and 400 connections
      Thread Stats   Avg      Stdev     Max   +/- Stdev
        Latency     5.69ms    5.96ms  97.21ms   89.25%
        Req/Sec    20.52k     2.09k   30.94k    69.35%
      2453618 requests in 30.10s, 301.85MB read
    Requests/sec:  81528.22
    Transfer/sec:     10.03MB

go (v1.16.2) with fasthttp (v1.31.0):

    Running 30s test @ http://127.0.0.1:8080/
      4 threads and 400 connections
      Thread Stats   Avg      Stdev     Max   +/- Stdev
        Latency     3.23ms    3.22ms  68.38ms   89.92%
        Req/Sec    33.72k     5.13k   70.38k    68.15%
      4024910 requests in 30.07s, 575.77MB read
    Requests/sec: 133834.23
    Transfer/sec:     19.15MB

At the moment, in the Windows environment, anweb works with only one worker thread.

### License

Licensed under either of
* Apache License, Version 2.0 (LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license (LICENSE-MIT or http://opensource.org/licenses/MIT) at your option.

at your option.
