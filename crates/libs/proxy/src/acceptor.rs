// pub trait Stream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin {}

// TODO: these abstraction might not workout

// Acceptor for tls
#[trait_variant::make(TlsAcceptor: Send)]
pub trait LocalTlsAcceptor: Clone {
    async fn accept(
        &self,
        stream: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + std::marker::Send,
    ) -> std::io::Result<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>;
}

#[derive(Clone)]
pub struct RustTlsAcceptor {
    tls: tokio_rustls::TlsAcceptor,
}

impl TlsAcceptor for RustTlsAcceptor {
    async fn accept(
        &self,
        stream: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + std::marker::Send,
    ) -> std::io::Result<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin> {
        self.tls.accept(stream).await
    }
}

impl RustTlsAcceptor {
    pub fn new(tls: tokio_rustls::TlsAcceptor) -> Self {
        Self { tls }
    }
}

// tcp or unix acceptor

// Acceptor for tls or unix
#[trait_variant::make(StreamAcceptor: Send)]
pub trait LocalStreamAcceptor {
    async fn accept(
        &self,
    ) -> std::io::Result<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send>;
}

pub struct TcpAccaptor {
    tcp: tokio::net::TcpListener,
}

impl StreamAcceptor for TcpAccaptor {
    async fn accept(
        &self,
    ) -> std::io::Result<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send> {
        let (stream, _) = self.tcp.accept().await?;
        Ok(stream)
    }
}
