fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    use std::{process::Command, sync::Arc, time::Duration};

    use bytes::BytesMut;
    use rustls::{
        pki_types,
        server::{ClientHello, ResolvesServerCert},
        sign::CertifiedKey,
        ClientConfig, RootCertStore,
    };
    // use rustls::server::ResolvesServerCert;
    use rustls_cng::{
        cert::CertContext,
        signer::CngSigningKey,
        store::{CertStore, CertStoreType},
    };
    use rustls_symcrypt::default_symcrypt_provider;
    use tokio::{
        io::{self, copy, sink, split, AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        sync::oneshot,
    };
    use tokio_rustls::{TlsAcceptor, TlsConnector};

    fn get_test_cert_hash() -> String {
        let output = Command::new("pwsh.exe")
            .args(["-Command", "Get-ChildItem Cert:\\CurrentUser\\My | Where-Object -Property FriendlyName -EQ -Value MsQuic-Test | Select-Object -ExpandProperty Thumbprint -First 1"]).
            output().expect("Failed to execute command");
        assert!(output.status.success());
        let mut s = String::from_utf8(output.stdout).unwrap();
        if s.ends_with('\n') {
            s.pop();
            if s.ends_with('\r') {
                s.pop();
            }
        };
        s
    }

    fn get_cert_hash_bytes(hash: String) -> [u8; 20] {
        let mut hash_array: [u8; 20] = [0; 20];
        hex::decode_to_slice(hash.as_bytes(), &mut hash_array).expect("Decoding failed");
        hash_array
    }

    #[derive(Debug)]
    pub struct ServerCertResolver {
        inner: Vec<CertContext>,
    }

    impl ResolvesServerCert for ServerCertResolver {
        fn resolve(&self, client_hello: ClientHello) -> Option<Arc<CertifiedKey>> {
            println!("Client hello server name: {:?}", client_hello.server_name());
            //let name = client_hello.server_name()?;

            // look up certificate by subject
            // let contexts = self.0.find_by_subject_str(name).ok()?;
            let contexts = &self.inner;

            // attempt to acquire a private key and construct CngSigningKey
            let (context, key) = contexts.into_iter().find_map(|ctx| {
                let key = ctx.acquire_key().ok()?;
                CngSigningKey::new(key).ok().map(|key| (ctx, key))
            })?;

            println!("Key alg group: {:?}", key.key().algorithm_group());
            println!("Key alg: {:?}", key.key().algorithm());

            // attempt to acquire a full certificate chain
            let chain = context.as_chain_der().ok()?;
            let certs = chain.into_iter().map(Into::into).collect();

            // return CertifiedKey instance
            Some(Arc::new(CertifiedKey {
                cert: certs,
                key: Arc::new(key),
                ocsp: None,
            }))
        }
    }

    #[test]
    fn basic() {
        let hash_str = get_test_cert_hash();
        let hash_bytes = get_cert_hash_bytes(hash_str);
        // run server
        let store = CertStore::open(CertStoreType::CurrentUser, "My").unwrap();
        // find test cert
        // find by subject name is somehow not working.
        //let cert = store.find_by_subject_name("CN=YYDEV").unwrap().first().unwrap();

        let contexts = store.find_by_sha1(hash_bytes).unwrap();
        let contexts_copy = contexts.clone();
        // make resolver
        let cert_resolver = ServerCertResolver { inner: contexts };

        let config =
            rustls::ServerConfig::builder_with_provider(Arc::new(default_symcrypt_provider()))
                //.with_no_client_auth()
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_no_client_auth()
                .with_cert_resolver(Arc::new(cert_resolver));

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

                        // if flag_echo {
                        //     let (mut reader, mut writer) = split(stream);
                        //     let n = copy(&mut reader, &mut writer).await?;
                        //     writer.flush().await?;
                        //     println!("Echo: {} - {}", peer_addr, n);
                        // } else {
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
}
