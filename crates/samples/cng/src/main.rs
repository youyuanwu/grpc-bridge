#[cfg(test)]
mod test;

// copies tls unencrypted data to uds
// TODO: this fails at http2 negotiation, we need to use hyper.
#[tokio::main]
async fn main() {
    //let (config, _) = proxy::test_util::load_test_server_config();
    let addr = "127.0.0.1:5047";
    println!("start proxy at {addr}");
    proxy::serve_proxy(addr.parse().unwrap()).await.unwrap();

    println!("server end")
}
