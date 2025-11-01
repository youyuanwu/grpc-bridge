#[cfg(windows)]
#[cfg(test)]
mod test;

#[cfg(windows)]
pub mod util;

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
    use yarrp::CancellationToken;

    let addr = "127.0.0.1:5047";
    println!("start proxy at {addr}");
    let token = CancellationToken::new();
    util::serve_proxy(addr.parse().unwrap(), token)
        .await
        .unwrap();

    println!("server end")
}

#[cfg(unix)]
async fn unix_main() {
    panic!("unix net supported yet");
}
