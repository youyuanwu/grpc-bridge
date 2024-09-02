use std::{process::Command, sync::Arc};

use rustls::{
    server::{ClientHello, ResolvesServerCert},
    sign::CertifiedKey,
};
use rustls_cng::{
    cert::CertContext,
    signer::CngSigningKey,
    store::{CertStore, CertStoreType},
};
use rustls_symcrypt::default_symcrypt_provider;

/// returns the server config for hyper server
/// and the context for client to use keys.
pub fn load_test_server_config() -> (rustls::ServerConfig, Vec<CertContext>) {
    let contexts = ServerCertResolver::get_cert_context();
    let contexts_copy = contexts.clone();
    let cert_resolver = ServerCertResolver::create_default(contexts);

    let config = rustls::ServerConfig::builder_with_provider(Arc::new(default_symcrypt_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(cert_resolver));
    (config, contexts_copy)
}

// always return the same key.
// It is possible to return different key based on client.
#[derive(Debug)]
pub struct ServerCertResolver {
    key: Arc<CertifiedKey>,
}

impl ResolvesServerCert for ServerCertResolver {
    fn resolve(&self, client_hello: ClientHello) -> Option<Arc<CertifiedKey>> {
        println!("Client hello server name: {:?}", client_hello.server_name());
        Some(self.key.clone())
    }
}

impl ServerCertResolver {
    fn get_cert_context() -> Vec<CertContext> {
        let hash_str = get_test_cert_hash();
        let hash_bytes = get_cert_hash_bytes(hash_str);
        // run server
        let store = CertStore::open(CertStoreType::CurrentUser, "My").unwrap();
        // find test cert
        // find by subject name is somehow not working.
        //let contexts = store.find_by_subject_name("CN=YYDEV").unwrap();

        let contexts = store.find_by_sha1(hash_bytes).unwrap();
        assert!(!contexts.is_empty());
        contexts
    }
    fn create_default(contexts: Vec<CertContext>) -> Self {
        // attempt to acquire a private key and construct CngSigningKey
        let (context, key) = contexts
            .into_iter()
            .find_map(|ctx| {
                let key = ctx.acquire_key().ok()?;
                CngSigningKey::new(key).ok().map(|key| (ctx, key))
            })
            .unwrap();

        println!("Key alg group: {:?}", key.key().algorithm_group());
        println!("Key alg: {:?}", key.key().algorithm());

        // attempt to acquire a full certificate chain
        let chain = context.as_chain_der().ok().unwrap();
        let certs = chain.into_iter().map(Into::into).collect();
        let key_der = CertifiedKey::new(certs, Arc::new(key));
        // cannot use the with single cert API since we use a custom key provider.
        // make resolver
        ServerCertResolver {
            key: Arc::new(key_der),
        }
    }
}

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
