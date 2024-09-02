use std::future::Future;
use std::os::windows::io::IntoRawSocket;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::{os::windows::io::FromRawSocket, path::PathBuf, sync::Arc};

use hyper::Uri;
use hyper_util::rt::TokioIo;
// fix connector
#[derive(Debug, Clone)]
pub struct UdsConnector {
    path: Arc<PathBuf>,
}

impl UdsConnector {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path: Arc::new(path),
        }
    }
    pub async fn connect(&self) -> std::io::Result<TokioIo<tokio::net::TcpStream>> {
        let path = self.path.as_ref().to_owned();
        let unix_stream =
            tokio::task::spawn_blocking(move || uds_windows::UnixStream::connect(path)).await??;
        // We need to do this sometime before `tokio::net::TcpStream::from_std()` is
        // called.
        unix_stream.set_nonblocking(true).unwrap();
        // Create a std::net::TcpStream from the raw Unix socket. Windows APIs that
        // accept sockets have defined behavior for Unix sockets (either they
        // successfully handle it or return an error), so this should be safe.
        let std_stream =
            unsafe { std::net::TcpStream::from_raw_socket(unix_stream.into_raw_socket()) };
        let tokio_tcp_stream = tokio::net::TcpStream::from_std(std_stream)?;
        Ok(TokioIo::new(tokio_tcp_stream))
    }
}

impl Unpin for UdsConnector {}

impl tower::Service<hyper::Uri> for UdsConnector {
    type Response = TokioIo<tokio::net::TcpStream>;
    type Error = std::io::Error;
    #[allow(clippy::type_complexity)]
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn call(&mut self, _req: Uri) -> Self::Future {
        let conn = self.clone();
        let fut = async move { conn.connect().await };

        Box::pin(fut)
    }

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
}
