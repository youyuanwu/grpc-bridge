// reference: https://github.com/stefansundin/hyper-reverse-proxy/tree/master

use std::{error::Error, net::SocketAddr, sync::Arc};

use crate::conn::UdsConnector;
use crate::uds_service::UdsService;
use tokio::{net::TcpListener, select};

pub use tokio_util::sync::CancellationToken;

/// Serves the proxy on the addr
pub async fn serve_proxy(
    addr: SocketAddr,
    token: CancellationToken,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Starting to serve on https://{}", addr);

    // Create a TCP listener via tokio.
    let incoming = TcpListener::bind(&addr).await?;

    // Build TLS configuration.
    let (mut server_config, _) = crate::test_util::load_test_server_config();
    server_config.alpn_protocols = vec![b"h2".to_vec()]; // b"http/1.1".to_vec(), b"http/1.0".to_vec()
    let tls_acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));

    let test_socket = crate::test_util::get_test_socket_path();
    let conn = UdsConnector::new(test_socket);
    let service = UdsService::new(conn).await;
    loop {
        let svc_cp = service.clone();

        let (tcp_stream, _remote_addr) = select! {
            x = incoming.accept() => { x?}
            _ = token.cancelled() => {
                println!("proxy cancelled");
                break Ok(());
            }
        };

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
                    .serve_connection(hyper_util::rt::TokioIo::new(tls_stream), svc_cp)
                    .await
            {
                if let Some(e) = err.downcast_ref::<hyper::Error>() {
                    if let Some(s) = e.source() {
                        if let Some(ss) = s.downcast_ref::<std::io::Error>() {
                            if ss.kind() == std::io::ErrorKind::ConnectionReset {
                                // client closed connection
                                // TODO: maybe some read is not finished (in dotnet).
                                return;
                            }
                            eprintln!("failed to serve connection io error: {ss:#}");
                        }
                    }
                    eprintln!("failed to serve connection hyper error: {e:#}");
                } else {
                    eprintln!("failed to serve connection general error: {err:#}");
                }
            }
        });
    }
}

#[cfg(test)]
mod test {

    use tokio_util::sync::CancellationToken;

    use crate::serve_proxy;

    #[tokio::test]
    async fn e2e_test() {
        // open cpp server
        let curr_dir = std::env::current_dir().unwrap();
        println!("{:?}", curr_dir);
        let root_dir = curr_dir
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let server_exe = root_dir.join("build/examples/helloworld/Debug/greeter_server.exe");
        println!("launching {:?}", server_exe);
        let mut child_server = std::process::Command::new(server_exe.as_path())
            .spawn()
            .expect("Couldn't run server");

        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        let token = CancellationToken::new();
        let token_cp = token.clone();
        // open proxy
        let proxy_child = tokio::spawn(async {
            let addr = "127.0.0.1:5047";
            println!("start proxy at {addr}");
            serve_proxy(addr.parse().unwrap(), token_cp).await.unwrap();
        });

        // proxy server might be slow to come up.
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        // send csharp request to server
        println!("launching csharp client");
        let mut child_client = std::process::Command::new("dotnet.exe")
            .current_dir(root_dir)
            .args([
                "run",
                "--project",
                "./src/greeter_client/greeter_client.csproj",
            ])
            .spawn()
            .expect("Couldn't run client");

        tokio::task::spawn_blocking(move || {
            child_client.wait().expect("client failed");
        })
        .await
        .unwrap();

        // stop proxy
        token.cancel();
        proxy_child.await.unwrap();

        // stop cpp server
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        child_server.kill().expect("!kill");
        child_server.wait().unwrap();
    }
}
