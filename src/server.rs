mod proxy;

use crate::config::AppConfig;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::error::Error;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::TcpListener;

pub async fn start_server() -> Result<(), Box<dyn Error + Send + Sync>> {
    let app_config = AppConfig::instance();
    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
    let addr = SocketAddr::new(ip, app_config.port);
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, _) = listener.accept().await?;

        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(proxy::proxy_service))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}
