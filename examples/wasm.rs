use anweb::server::{Event, Server};
use std::fs::File;
use std::io::Read;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let wasm_file_data = Arc::new(read_wasm_file()?);

    let addr = ([0, 0, 0, 0], 8080).into();

    let server = Server::new(&addr)?;
    server.run(move |server_event| {
        if let Event::Connected(tcp_session) = server_event {
            let wasm_file_data = wasm_file_data.clone();
            tcp_session.to_http(move |http_result| {
                let request = http_result?;
                match request.path() {
                    "/" => {
                        request.response(200).html(INDEX_HTML).send();
                    }
                    "/simple.wasm" => {
                        request.response(200).wasm(&wasm_file_data).send();
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

fn read_wasm_file() -> Result<Vec<u8>, std::io::Error> {
    let mut file = File::open("examples/simple_wasm/target/wasm32-unknown-unknown/release/simple.wasm").expect(
        "Laod wasm file error. {}.\n\
                 May be you need build wasm file. Go to \"examples/simple_wasm\" and make\n\
                 cargo build --target wasm32-unknown-unknown --release\n\
                 may be will need install target:\n\
                 rustup update\n\
                 rustup target add wasm32-unknown-unknown --toolchain stable",
    );

    let mut wasm_file_data = vec![];
    file.read_to_end(&mut wasm_file_data)?;
    Ok(wasm_file_data)
}

const INDEX_HTML: &str = r#"
<html>
    <body>
        <script>
            var importObj = {
                imports: { imported_func: arg => {} }
            };

            var prog = null
            WebAssembly.instantiateStreaming(fetch('simple.wasm'), importObj).then(new_prog => {
                prog = new_prog
            });

            function calcInWasm() {
                number1 = document.getElementById('number1').value
                number2 = document.getElementById('number2').value
                document.getElementById('wasmResult').innerHTML = prog.instance.exports.sum(number1, number2);
            }
        </script>

    	<h3>Wasm example</h3>
        <input type="number" id="number1" value="2"/> + <input type="number" id="number2" value="2"/> = <b id="wasmResult"/> <br>
        <form onsubmit="calcInWasm(); return false;">
            <button type="submit">Calc</button>
        </form>
     </body>
</html>
"#;
