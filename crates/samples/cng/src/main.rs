#[cfg(windows)]
#[cfg(test)]
mod test;
// copies tls unencrypted data to uds
#[tokio::main]
async fn main() {
    #[cfg(unix)]
    unix_main().await;

    #[cfg(windows)]
    win_main().await;
}

#[cfg(windows)]
async fn win_main() {
    let addr = "127.0.0.1:5047";
    println!("start proxy at {addr}");
    proxy::serve_proxy(addr.parse().unwrap(), proxy::CancellationToken::new())
        .await
        .unwrap();

    println!("server end")
}

#[cfg(unix)]
async fn unix_main() {
    panic!("unix net supported yet");
}
