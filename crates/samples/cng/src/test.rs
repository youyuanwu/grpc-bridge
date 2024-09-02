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
    let (config, contexts_copy) = proxy::test_util::load_test_server_config();
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
                    //}

                    Ok(()) as io::Result<()>
                };

                tokio::spawn(async move {
                    if let Err(err) = fut.await {
                        eprintln!("{:?}", err);
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
        let client_config =
            ClientConfig::builder_with_provider(Arc::new(default_symcrypt_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_root_certificates(root_store)
                .with_no_client_auth();
        // .with_root_certificates(root_store)
        // .with_client_cert_resolver(Arc::new(ClientCertResolver(
        //     store,
        //     params.client_cert.clone(),
        // )));
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