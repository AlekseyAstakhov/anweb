use anweb::http_session::{HttpSession, HttpError};
use anweb::cookie::Cookie;
use anweb::query::{parse_query, Query};
use anweb::request::Request;
use anweb::server::{Event, Server};
use rand::prelude::*;
use std::collections::hash_map::HashMap;
use std::sync::{Arc, Mutex};

const SESSION_ID_COOKIE_NAME: &str = "session_id";
struct User {}
type Users = Arc<Mutex<HashMap<String /*session id*/, User>>>;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let users = Arc::new(Mutex::new(HashMap::new()));
    let addr = ([0, 0, 0, 0], 8080).into();
    let server = Server::new(&addr)?;
    server.run(move |server_event| {
        if let Event::Connected(tcp_session) = server_event {
            let users = users.clone();
            tcp_session.upgrade_to_http(move |http_result, http_session| {
                let request = http_result?;
                if let Some(session_id) = session_id_from_request(request) {
                    if is_logged(&session_id, &users) {
                        response_for_logged_user(&http_session, request, &users, &session_id);
                        return Ok(());
                    }
                }

                response_for_unlogged_user(&http_session, request, &users)?;

                Ok(())
            });
        }
    })?;

    Ok(())
}

fn response_for_unlogged_user(http_session: &HttpSession, request: &Request, users: &Users) -> Result<(), HttpError> {
    match request.path() {
        "/" => {
            http_session.response_200_html(LOGIN_PAGE, request);
        }
        "/login" => {
            if let Some(content_len) = request.content_len() {
                if content_len < 256 {
                    let users = users.clone();
                    let request = request.clone();
                    let mut content = vec![];
                    http_session.read_content(move |data, done, mut http_session| {
                        content.extend_from_slice(data);
                        if done {
                            let form = parse_query(&content);
                            response_to_login_form(&mut http_session, &request, &form, &users);
                        }
                        Ok(())
                    })
                } else {
                    dbg!("a lot of data for login and password");
                    http_session.response_400_text("A lot of data for login and password. Bye bye.", request);
                    http_session.disconnect();
                }
            }
        }
        _ => {
            http_session.response_404_text("404 page not found", request);
        }
    }

    Ok(())
}

fn response_to_login_form(http_session: &mut HttpSession, request: &Request, query: &Query, users: &Users) {
    if query.value("login").unwrap_or("".to_string()) == "admin" && query.value("password").unwrap_or("".to_string()) == "admin" {
        let session_id = generate_session_id();

        {
            let mut users = users.lock().unwrap();
            users.insert(session_id.clone(), User {});
        }

        let cookie = Cookie {
            name: "session_id",
            value: &session_id,
            http_only: true,
            path: None,
            domain: None,
            expires: None,
            max_age: None,
            secure: false,
        };
        http_session.response_303_with_cookie("/", &cookie, request);
        return;
    }

    http_session.response_200_html(AUTHENTICATION_FAILED_PAGE, request);
}

fn response_for_logged_user(http_session: &HttpSession, request: &Request, users: &Users, session_id: &str) {
    match request.path() {
        "/" => {
            http_session.response_200_html(LOGGED_USER_PAGE, request);
        }
        "/logout" => {
            if let Ok(mut users) = users.lock() {
                users.remove(session_id);
            }

            let cookie = Cookie::remove("session_id");
            http_session.response_303_with_cookie("/", &cookie, request);
        }
        _ => {
            http_session.response_404_text("404 page not found", request);
        }
    }
}

fn session_id_from_request(request: &Request) -> Option<String> {
    if let Some(session_cookie) = request.cookies().iter().find(|cookie| cookie.name == SESSION_ID_COOKIE_NAME) {
        return Some(session_cookie.value.to_string());
    }

    None
}

fn is_logged(session_id: &str, users: &Users) -> bool {
    if let Ok(users) = users.lock() {
        if users.contains_key(session_id) {
            return true;
        }
    }

    false
}

fn generate_session_id() -> String {
    const LEN: usize = 48;
    let mut result = String::with_capacity(LEN);
    let mut rng = rand::thread_rng();
    for _ in 0..LEN {
        let ch = if rng.gen_range(0, 2) == 1 {
            rng.gen_range(b'A', b'Z' + 1)
        } else {
            rng.gen_range(b'a', b'z' + 1)
        };
        result.push(char::from(ch));
    }

    result
}

const LOGIN_PAGE: &str = r#"
<html>
    <body>
        <h3>Login-session example</h3>
        <form action="login" method="post">
            <input type="text" name="login" /> <br>
            <input type="password" name="password" /> <br>
            <button type="submit">Log In</button>
        </form>
    </body>
</html>
"#;

const AUTHENTICATION_FAILED_PAGE: &str = r#"
<html>
    <body>
        <b>Authentication failed.</b>
        <p>Hint: user is admin, password is admin.</p>
        <a href="/">
            <button type="submit">Try again</button>
        </a>
    </body>
</html>
"#;

const LOGGED_USER_PAGE: &str = r#"
<html>
    <body>
        <form action="logout">
            <button type="submit">Log out</button>
        </form>
    </body>
</html>
"#;
