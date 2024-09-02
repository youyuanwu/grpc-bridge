// reference: https://github.com/stefansundin/hyper-reverse-proxy/tree/master

use std::{net::SocketAddr, sync::Arc, time::Duration};

use hyper::{body::Incoming, Request, Response};
use hyper_util::{client::legacy::Client, rt::TokioTimer};
use tokio::net::TcpListener;
use tower::Service;

pub mod conn;
pub mod test_util;

/// Serves the proxy on the addr
pub async fn serve_proxy(addr: SocketAddr) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Starting to serve on https://{}", addr);

    // Create a TCP listener via tokio.
    let incoming = TcpListener::bind(&addr).await?;

    // Build TLS configuration.
    let (mut server_config, _) = test_util::load_test_server_config();
    server_config.alpn_protocols = vec![b"h2".to_vec()]; // b"http/1.1".to_vec(), b"http/1.0".to_vec()
    let tls_acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));

    let service = hyper::service::service_fn(handle);

    loop {
        let (tcp_stream, _remote_addr) = incoming.accept().await?;

        let tls_acceptor = tls_acceptor.clone();
        tokio::spawn(async move {
            let tls_stream = match tls_acceptor.accept(tcp_stream).await {
                Ok(tls_stream) => tls_stream,
                Err(err) => {
                    eprintln!("failed to perform tls handshake: {err:#}");
                    return;
                }
            };
            if let Err(err) =
                hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new())
                    .serve_connection(hyper_util::rt::TokioIo::new(tls_stream), service)
                    .await
            {
                eprintln!("failed to serve connection: {err:#}");
            }
        });
    }
}

async fn handle(
    req: Request<Incoming>,
) -> Result<Response<Incoming>, hyper_util::client::legacy::Error> {
    println!("handle request. {:?}", req);
    let mut c = make_client::<Incoming>().await;
    println!("calling client.");
    c.call(req).await.inspect_err(|e| println!("error: {e}"))
}

async fn make_client<R: hyper::body::Body + Send>() -> Client<conn::UdsConnector, R>
where
    <R as hyper::body::Body>::Data: Send,
{
    let sock = "my.sock";
    let path = std::env::temp_dir().join(sock);
    let conn = conn::UdsConnector::new(path);
    hyper_util::client::legacy::Builder::new(hyper_util::rt::TokioExecutor::new())
        .pool_idle_timeout(Duration::from_secs(3))
        .pool_timer(TokioTimer::new())
        .http2_only(true)
        .build::<_, R>(conn)
}

#[cfg(test)]
mod test {
    use http_body_util::Full;
    use hyper::{body::Bytes, Uri};

    use crate::make_client;

    #[tokio::test]
    #[ignore]
    async fn test_uds_connect() {
        //let uri_str = "unix://C:/Users/user1/AppData/Local/Temp/my.sock";
        // This can get a connection.
        let sock = "my.sock";
        let path = std::env::temp_dir().join(sock);
        let uri_str = hyperlocal_with_windows::Uri::new(path.as_path().to_str().unwrap(), "");
        let unix_url: Uri = uri_str.into();
        let c = make_client::<Full<Bytes>>().await;
        c.get(unix_url).await.unwrap();
    }
}
