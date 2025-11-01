use std::{sync::Arc, time::Duration};

use bytes::BytesMut;
use rustls::{pki_types, ClientConfig, RootCertStore};
use rustls_symcrypt::default_symcrypt_provider;
use tokio::{
    io::{self, copy, sink, split, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};
use tokio_rustls::{TlsAcceptor, TlsConnector};

#[test]
fn basic() {
    let (config, contexts_copy) = yarrp_rustls::test_util::load_test_server_config();
    // run tokio server
    let acceptor = TlsAcceptor::from(Arc::new(config));

    let (sht_tx, mut sht_rx) = oneshot::channel::<()>();
    // run server in a thread.
    let th = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .enable_io()
            .build()
            .unwrap();
        rt.block_on(async {
            let addr = "127.0.0.1:2345";
            let listener = TcpListener::bind(&addr).await.unwrap();

            loop {
                let (stream, peer_addr) = tokio::select! {
                    val = listener.accept() => val,
                    _ = &mut sht_rx => {
                        println!("server accepted interrupted.");
                        break; // stop accept and break.
                    }
                }
                .unwrap();

                let acceptor = acceptor.clone();
                println!("tcp accepted from {}", peer_addr);
                let fut = async move {
                    let mut stream = acceptor.accept(stream).await?;
                    let mut output = sink();
                    stream
                        .write_all(
                            &b"HTTP/1.0 200 ok\r\n\
                                Connection: close\r\n\
                                Content-length: 12\r\n\
                                \r\n\
                                Hello world!"[..],
                        )
                        .await?;
                    stream.shutdown().await?;
                    copy(&mut stream, &mut output).await?;
                    println!("Hello: {}", peer_addr);
                    Ok(()) as io::Result<()>
                };

                tokio::spawn(async move {
                    if let Err(err) = fut.await {
                        eprintln!("server tls stream error {}", err);
                    }
                });
            }
            println!("server end")
        });
    });

    std::thread::sleep(Duration::from_secs(1));

    // run client on current thread
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .enable_io()
        .build()
        .unwrap();
    rt.block_on(async move {
        let mut root_store = RootCertStore::empty();
        root_store
            .add(contexts_copy.first().unwrap().as_der().into())
            .unwrap();
        let client_cert =
            yarrp_rustls::cng::ClientCertResolver::try_from_certs(contexts_copy).unwrap();
        let client_config =
            ClientConfig::builder_with_provider(Arc::new(default_symcrypt_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_root_certificates(root_store)
                .with_client_cert_resolver(Arc::new(client_cert));
        let addr = "127.0.0.1:2345";
        let connector = TlsConnector::from(Arc::new(client_config));
        let stream = TcpStream::connect(&addr).await.unwrap();

        //let domain = options.domain.unwrap_or(options.host);
        let content = format!("GET / HTTP/1.0\r\nHost: {}\r\n\r\n", "localhost");
        let domain = pki_types::ServerName::try_from("localhost")
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid dnsname"))
            .unwrap()
            .to_owned();

        let mut stream = connector.connect(domain, stream).await.unwrap();
        stream.write_all(content.as_bytes()).await.unwrap();
        let (mut reader, mut writer) = split(stream);
        let mut buffer = BytesMut::with_capacity(50);
        let len = reader.read_buf(&mut buffer).await.unwrap();
        let reply = String::from_utf8_lossy(&buffer[..len]).into_owned();
        println!("The bytes: {:?}", reply);
        writer.shutdown().await.unwrap();
    });

    sht_tx.send(()).unwrap();
    th.join().unwrap();
}

#[cfg(test)]
mod proxy_test {

    use std::path::Path;

    use tokio_util::sync::CancellationToken;

    use crate::util::serve_proxy;

    async fn invoke_csharp_client(root_dir: &Path) {
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
            // call it twice
            child_client.wait().expect("client failed");
        })
        .await
        .unwrap();
    }

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

        // call it twice
        invoke_csharp_client(root_dir).await;
        invoke_csharp_client(root_dir).await;

        // stop proxy
        token.cancel();
        proxy_child.await.unwrap();

        // stop cpp server
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        child_server.kill().expect("!kill");
        child_server.wait().unwrap();
    }
}
