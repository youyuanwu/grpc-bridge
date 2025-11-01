use std::{net::SocketAddr, sync::Arc};

use tokio::net::TcpListener;
use yarrp::{connector::UdsConnector, proxy_service::ProxyService, CancellationToken};
use yarrp_rustls::accept_stream::RustlsAcceptStream;

/// Serves the proxy on the addr
pub async fn serve_proxy(
    addr: SocketAddr,
    token: CancellationToken,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Starting to serve on https://{}", addr);

    // Create a TCP listener via tokio.
    let incoming = TcpListener::bind(&addr).await?;

    // Build TLS configuration.
    let (mut server_config, _) = yarrp_rustls::test_util::load_test_server_config();
    server_config.alpn_protocols = vec![b"h2".to_vec()]; // b"http/1.1".to_vec(), b"http/1.0".to_vec()
    let tls_acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));

    let test_socket = yarrp_rustls::test_util::get_test_socket_path();
    let conn = UdsConnector::new(test_socket);
    let service = ProxyService::new(conn);

    let rustls_accept_stream = RustlsAcceptStream::new(incoming, tls_acceptor, None);

    yarrp::serve_with_incoming(rustls_accept_stream, service, async move {
        token.cancelled().await
    })
    .await?;
    Ok(())
}
