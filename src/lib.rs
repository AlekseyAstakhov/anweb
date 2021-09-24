#![forbid(unsafe_code)]

pub mod tcp_session;
pub mod http_result;
pub mod cookie;
pub mod tls;
pub mod mime;
pub mod multipart;
pub mod query;
pub mod redirect_server;
pub mod request;
pub mod response;
pub mod server;
pub mod static_files;
pub mod websocket;
pub mod worker;
mod web_session;
mod request_parser;
