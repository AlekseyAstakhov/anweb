#![forbid(unsafe_code)]

pub mod tcp_client;
pub mod http_client;
pub mod websocket_client;
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
mod connection;
mod request_parser;
mod content_loader;
